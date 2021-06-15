// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference no-default-lib="true" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const Deno = window.Deno;
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

  // TODO(lucacasonato): this needs to not be hardcoded and instead depend on
  // host os.
  const isWindows = false;
  const POOL_SIZE = 65536;

  /**
   * @param {string} input
   * @param {number} position
   * @returns {{result: string, position: number}}
   */
  function collectCodepointsNotCRLF(input, position) {
    // See https://w3c.github.io/FileAPI/#convert-line-endings-to-native and
    // https://infra.spec.whatwg.org/#collect-a-sequence-of-code-points
    const start = position;
    for (
      let c = input.charAt(position);
      position < input.length && !(c === "\r" || c === "\n");
      c = input.charAt(++position)
    );
    return { result: input.slice(start, position), position };
  }

  /**
   * @param {string} s
   * @returns {string}
   */
  function convertLineEndingsToNative(s) {
    const nativeLineEnding = isWindows ? "\r\n" : "\n";

    let { result, position } = collectCodepointsNotCRLF(s, 0);

    while (position < s.length) {
      const codePoint = s.charAt(position);
      if (codePoint === "\r") {
        result += nativeLineEnding;
        position++;
        if (position < s.length && s.charAt(position) === "\n") {
          position++;
        }
      } else if (codePoint === "\n") {
        position++;
        result += nativeLineEnding;
      }
      const { result: token, position: newPosition } = collectCodepointsNotCRLF(
        s,
        position,
      );
      position = newPosition;
      result += token;
    }

    return result;
  }

  /** @param {(Blob | Uint8Array)[]} parts */
  async function * toIterator (parts) {
    for (const part of parts) {
      if (part instanceof Blob) {
        yield * part.stream();
      } else if (ArrayBuffer.isView(part)) {
        let position = part.byteOffset;
        const end = part.byteOffset + part.byteLength;
        while (position !== end) {
          const size = Math.min(end - position, POOL_SIZE);
          const chunk = part.buffer.slice(position, position + size);
          position += chunk.byteLength;
          yield new Uint8Array(chunk);
        }
      }
    }
  }

  /** @typedef {BufferSource | Blob | string} BlobPart */

  /**
   * @param {BlobPart[]} parts
   * @param {string} endings
   * @returns {{ parts: (Uint8Array|Blob)[], size: number }}
   */
  function processBlobParts(parts, endings) {
    /** @type {(Uint8Array|Blob)[]} */
    const parts = [];
    let size = 0;
    for (const element of parts) {
      if (element instanceof ArrayBuffer) {
        parts.push(new Uint8Array(element.slice(0)));
        size += element.byteLength;
      } else if (ArrayBuffer.isView(element)) {
        const buffer = element.buffer.slice(
          element.byteOffset,
          element.byteOffset + element.byteLength,
        );
        size += element.byteLength;
        parts.push(new Uint8Array(buffer));
      } else if (element instanceof Blob) {
        parts.push(element);
        size += element.size;
      } else if (typeof element === "string") {
        const chunk = core.encode(endings == "native" ? convertLineEndingsToNative(element) : element);
        size += chunk.byteLength;
        parts.push(chunk);
      } else {
        throw new TypeError("Unreachable code (invalid element type)");
      }
    }
    return {parts, size};
  }

  /**
   * @param {string} str
   * @returns {string}
   */
  function normalizeType(str) {
    let normalizedType = str;
    if (!/^[\x20-\x7E]*$/.test(str)) {
      normalizedType = "";
    }
    return normalizedType.toLowerCase();
  }

  class Blob {
    get [Symbol.toStringTag]() {
      return "Blob";
    }

    /** @type {string} */
    #type;

    /** @type {(Uint8Array|Blob)[]} */
    #parts;

    /**
     * @param {BlobPart[]} blobParts
     * @param {BlobPropertyBag} options
     */
    constructor(blobParts = [], options = {}) {
      const prefix = "Failed to construct 'Blob'";
      blobParts = webidl.converters["sequence<BlobPart>"](blobParts, {
        context: "Argument 1",
        prefix,
      });
      options = webidl.converters["BlobPropertyBag"](options, {
        context: "Argument 2",
        prefix,
      });

      this[webidl.brand] = webidl.brand;

      const {parts, size} = processBlobParts(
        blobParts,
        options.endings,
      )
      /** @type {Uint8Array|Blob} */
      this.#parts = parts;
      this[_Size] = size;
      this.#type = normalizeType(options.type);
    }

    /** @returns {number} */
    get size() {
      webidl.assertBranded(this, Blob);
      return this[_Size];
    }

    /** @returns {string} */
    get type() {
      webidl.assertBranded(this, Blob);
      return this.#type;
    }

    /**
     * @param {number} [start]
     * @param {number} [end]
     * @param {string} [contentType]
     * @returns {Blob}
     */
    slice(start, end, contentType) {
      webidl.assertBranded(this, Blob);
      const prefix = "Failed to execute 'slice' on 'Blob'";
      if (start !== undefined) {
        start = webidl.converters["long long"](start, {
          clamp: true,
          context: "Argument 1",
          prefix,
        });
      }
      if (end !== undefined) {
        end = webidl.converters["long long"](end, {
          clamp: true,
          context: "Argument 2",
          prefix,
        });
      }
      if (contentType !== undefined) {
        contentType = webidl.converters["DOMString"](contentType, {
          context: "Argument 3",
          prefix,
        });
      }

      // deno-lint-ignore no-this-alias
      const O = this;
      /** @type {number} */
      let relativeStart;
      if (start === undefined) {
        relativeStart = 0;
      } else {
        if (start < 0) {
          relativeStart = Math.max(O.size + start, 0);
        } else {
          relativeStart = Math.min(start, O.size);
        }
      }
      /** @type {number} */
      let relativeEnd;
      if (end === undefined) {
        relativeEnd = O.size;
      } else {
        if (end < 0) {
          relativeEnd = Math.max(O.size + end, 0);
        } else {
          relativeEnd = Math.min(end, O.size);
        }
      }

      const span = Math.max(relativeEnd - relativeStart, 0);
      const parts = this.#parts;
      const blobParts = [];
      let added = 0;

      for (const part of parts) {
        const size = ArrayBuffer.isView(part) ? part.byteLength : part.size;
        if (relativeStart && size <= relativeStart) {
          // Skip the beginning and change the relative
          // start & end position as we skip the unwanted parts
          relativeStart -= size;
          relativeEnd -= size;
        } else {
          let chunk
          if (ArrayBuffer.isView(part)) {
            chunk = part.subarray(relativeStart, Math.min(size, relativeEnd));
            added += chunk.byteLength
          } else {
            chunk = part.slice(relativeStart, Math.min(size, relativeEnd));
            added += chunk.size
          }
          blobParts.push(chunk);
          relativeStart = 0; // All next sequential parts should start at 0

          // don't add the overflow to new blobParts
          if (added >= span) {
            break;
          }
        }
      }

      /** @type {string} */
      let relativeContentType;
      if (contentType === undefined) {
        relativeContentType = "";
      } else {
        relativeContentType = normalizeType(contentType);
      }

      const blob = new Blob([], {type: relativeContentType});
      blob[_Size] = span;
      blob.#parts = blobParts;

      return blob;
    }

    /**
     * @returns {ReadableStream<Uint8Array>}
     */
    stream() {
      webidl.assertBranded(this, Blob);
      const partIterator = toIterator(this.#parts);
      const stream = new ReadableStream({
        type: "bytes",
        /** @param {ReadableByteStreamController} controller */
        async pull (controller) {
          const {value} = await partIterator.next();
          if (!value) return controller.close()
          controller.enqueue(value);
        },
      });
      return stream;
    }

    /**
     * @returns {Promise<string>}
     */
    async text() {
      webidl.assertBranded(this, Blob);
      const buffer = await this.arrayBuffer();
      return core.decode(new Uint8Array(buffer));
    }

    /**
     * @returns {Promise<ArrayBuffer>}
     */
    async arrayBuffer() {
      webidl.assertBranded(this, Blob);
      const stream = this.stream();
      const bytes = new Uint8Array(this.size);
      let offset = 0;

      for await (const chunk of stream) {
        bytes.set(chunk, offset);
        offset += chunk.byteLength;
      }
      return bytes.buffer;
    }
  }

  webidl.configurePrototype(Blob);

  webidl.converters["Blob"] = webidl.createInterfaceConverter("Blob", Blob);
  webidl.converters["BlobPart"] = (V, opts) => {
    // Union for ((ArrayBuffer or ArrayBufferView) or Blob or USVString)
    if (typeof V == "object") {
      if (V instanceof Blob) {
        return webidl.converters["Blob"](V, opts);
      }
      if (V instanceof ArrayBuffer || V instanceof SharedArrayBuffer) {
        return webidl.converters["ArrayBuffer"](V, opts);
      }
      if (ArrayBuffer.isView(V)) {
        return webidl.converters["ArrayBufferView"](V, opts);
      }
    }
    return webidl.converters["USVString"](V, opts);
  };
  webidl.converters["sequence<BlobPart>"] = webidl.createSequenceConverter(
    webidl.converters["BlobPart"],
  );
  webidl.converters["EndingType"] = webidl.createEnumConverter("EndingType", [
    "transparent",
    "native",
  ]);
  const blobPropertyBagDictionary = [
    {
      key: "type",
      converter: webidl.converters["DOMString"],
      defaultValue: "",
    },
    {
      key: "endings",
      converter: webidl.converters["EndingType"],
      defaultValue: "transparent",
    },
  ];
  webidl.converters["BlobPropertyBag"] = webidl.createDictionaryConverter(
    "BlobPropertyBag",
    blobPropertyBagDictionary,
  );

  const _Name = Symbol("[[Name]]");
  const _Size = Symbol("[[Size]]");
  const _LastModfied = Symbol("[[LastModified]]");

  class File extends Blob {
    get [Symbol.toStringTag]() {
      return "File";
    }

    /** @type {string} */
    [_Name];
    /** @type {number} */
    [_LastModfied];

    /**
     * @param {BlobPart[]} fileBits
     * @param {string} fileName
     * @param {FilePropertyBag} options
     */
    constructor(fileBits, fileName, options = {}) {
      const prefix = "Failed to construct 'File'";
      webidl.requiredArguments(arguments.length, 2, { prefix });

      fileBits = webidl.converters["sequence<BlobPart>"](fileBits, {
        context: "Argument 1",
        prefix,
      });
      fileName = webidl.converters["USVString"](fileName, {
        context: "Argument 2",
        prefix,
      });
      options = webidl.converters["FilePropertyBag"](options, {
        context: "Argument 3",
        prefix,
      });

      super(fileBits, options);

      /** @type {string} */
      this[_Name] = fileName;
      if (options.lastModified === undefined) {
        /** @type {number} */
        this[_LastModfied] = new Date().getTime();
      } else {
        /** @type {number} */
        this[_LastModfied] = options.lastModified;
      }
    }

    /** @returns {string} */
    get name() {
      webidl.assertBranded(this, File);
      return this[_Name];
    }

    /** @returns {number} */
    get lastModified() {
      webidl.assertBranded(this, File);
      return this[_LastModfied];
    }
  }

  webidl.configurePrototype(File);

  webidl.converters["FilePropertyBag"] = webidl.createDictionaryConverter(
    "FilePropertyBag",
    blobPropertyBagDictionary,
    [
      {
        key: "lastModified",
        converter: webidl.converters["long long"],
      },
    ],
  );

  /**
   * This is a blob backed up by a file on the disk
   * with minium requirement. Its wrapped around a Blob as a blobPart
   * so you have no direct access to this.
   *
   * @author Jimmy Wärting
   * @private
   */
  class BlobDataItem extends Blob {
    #path;
    #start;

    constructor(options) {
      super();
      this.#path = options.path;
      this.#start = options.start;
      this[_Size] = options.size;
      this.lastModified = options.lastModified;
    }

    /**
     * Slicing arguments is first validated and formatted
     * to not be out of range by Blob.prototype.slice
     */
    slice(start, end) {
      return new BlobDataItem({
        path: this.#path,
        lastModified: this.lastModified,
        size: end - start,
        start
      });
    }

    async * stream() {
      const {mtime} = await Deno.stat(this.#path)
      if (mtime > this.lastModified) {
        throw new DOMException('The requested file could not be read, ' +
        'typically due to permission problems that have occurred after ' +
        'a reference to a file was acquired.', 'NotReadableError');
      }
      if (this.size) {
        const r = await Deno.open(this.#path, { read: true });
        let length = this.size;
        await r.seek(this.#start, Deno.SeekMode.Start);
        while (length) {
          const p = new Uint8Array(Math.min(length, POOL_SIZE));
          length -= await r.read(p);
          yield p
        }
      }
    }
  }

  // TODO: Make this function public
  /** @returns {Promise<File>} */
  async function getFile (path, type = '') {
    const stat = await Deno.stat(path);
    const blobDataItem = new BlobDataItem({
      path,
      size: stat.size,
      lastModified: stat.mtime.getTime(),
      start: 0
    });

    // TODO: import basename?
    const file = new File([blobDataItem], basename(path), {
      type, lastModified: blobDataItem.lastModified
    });

    return file;
  }

  window.__bootstrap.file = {
    Blob,
    getFile, // TODO: expose somehow? Write doc?
    File,
  };
})(this);
