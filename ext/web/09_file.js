// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference no-default-lib="true" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const webidl = window.__bootstrap.webidl;
  const {
    ArrayBufferPrototype,
    ArrayBufferPrototypeSlice,
    ArrayBufferIsView,
    ArrayPrototypePush,
    Date,
    DatePrototypeGetTime,
    MathMax,
    MathMin,
    ObjectPrototypeIsPrototypeOf,
    RegExpPrototypeTest,
    StringPrototypeCharAt,
    StringPrototypeToLowerCase,
    StringPrototypeSlice,
    Symbol,
    SymbolFor,
    TypedArrayPrototypeSet,
    TypeError,
    Uint8Array,
  } = window.__bootstrap.primordials;
  const consoleInternal = window.__bootstrap.console;

  // TODO(lucacasonato): this needs to not be hardcoded and instead depend on
  // host os.
  const isWindows = false;

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
      let c = StringPrototypeCharAt(input, position);
      position < input.length && !(c === "\r" || c === "\n");
      c = StringPrototypeCharAt(input, ++position)
    );
    return { result: StringPrototypeSlice(input, start, position), position };
  }

  /**
   * @param {string} s
   * @returns {string}
   */
  function convertLineEndingsToNative(s) {
    const nativeLineEnding = isWindows ? "\r\n" : "\n";

    let { result, position } = collectCodepointsNotCRLF(s, 0);

    while (position < s.length) {
      const codePoint = StringPrototypeCharAt(s, position);
      if (codePoint === "\r") {
        result += nativeLineEnding;
        position++;
        if (
          position < s.length && StringPrototypeCharAt(s, position) === "\n"
        ) {
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

  /** @param {(BlobReference | Blob)[]} parts */
  async function* toIterator(parts) {
    for (const part of parts) {
      yield* part.stream();
    }
  }

  /** @typedef {BufferSource | Blob | string} BlobPart */

  /**
   * @param {BlobPart[]} parts
   * @param {string} endings
   * @returns {{ parts: (BlobReference|Blob)[], size: number }}
   */
  function processBlobParts(parts, endings) {
    /** @type {(BlobReference|Blob)[]} */
    const processedParts = [];
    let size = 0;
    for (const element of parts) {
      if (ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, element)) {
        const chunk = new Uint8Array(ArrayBufferPrototypeSlice(element, 0));
        ArrayPrototypePush(processedParts, BlobReference.fromUint8Array(chunk));
        size += element.byteLength;
      } else if (ArrayBufferIsView(element)) {
        const chunk = new Uint8Array(
          element.buffer,
          element.byteOffset,
          element.byteLength,
        );
        size += element.byteLength;
        ArrayPrototypePush(processedParts, BlobReference.fromUint8Array(chunk));
      } else if (ObjectPrototypeIsPrototypeOf(BlobPrototype, element)) {
        ArrayPrototypePush(processedParts, element);
        size += element.size;
      } else if (typeof element === "string") {
        const chunk = core.encode(
          endings == "native" ? convertLineEndingsToNative(element) : element,
        );
        size += chunk.byteLength;
        ArrayPrototypePush(processedParts, BlobReference.fromUint8Array(chunk));
      } else {
        throw new TypeError("Unreachable code (invalid element type)");
      }
    }
    return { parts: processedParts, size };
  }

  /**
   * @param {string} str
   * @returns {string}
   */
  function normalizeType(str) {
    let normalizedType = str;
    if (!RegExpPrototypeTest(/^[\x20-\x7E]*$/, str)) {
      normalizedType = "";
    }
    return StringPrototypeToLowerCase(normalizedType);
  }

  /**
   * Get all Parts as a flat array containing all references
   * @param {Blob} blob
   * @param {string[]} bag
   * @returns {string[]}
   */
  function getParts(blob, bag = []) {
    for (const part of blob[_parts]) {
      if (ObjectPrototypeIsPrototypeOf(BlobPrototype, part)) {
        getParts(part, bag);
      } else {
        ArrayPrototypePush(bag, part._id);
      }
    }
    return bag;
  }

  const _type = Symbol("Type");
  const _size = Symbol("Size");
  const _parts = Symbol("Parts");

  class Blob {
    [_type] = "";
    [_size] = 0;
    [_parts];

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

      const { parts, size } = processBlobParts(
        blobParts,
        options.endings,
      );

      this[_parts] = parts;
      this[_size] = size;
      this[_type] = normalizeType(options.type);
    }

    /** @returns {number} */
    get size() {
      webidl.assertBranded(this, BlobPrototype);
      return this[_size];
    }

    /** @returns {string} */
    get type() {
      webidl.assertBranded(this, BlobPrototype);
      return this[_type];
    }

    /**
     * @param {number} [start]
     * @param {number} [end]
     * @param {string} [contentType]
     * @returns {Blob}
     */
    slice(start = undefined, end = undefined, contentType = undefined) {
      webidl.assertBranded(this, BlobPrototype);
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
          relativeStart = MathMax(O.size + start, 0);
        } else {
          relativeStart = MathMin(start, O.size);
        }
      }
      /** @type {number} */
      let relativeEnd;
      if (end === undefined) {
        relativeEnd = O.size;
      } else {
        if (end < 0) {
          relativeEnd = MathMax(O.size + end, 0);
        } else {
          relativeEnd = MathMin(end, O.size);
        }
      }

      const span = MathMax(relativeEnd - relativeStart, 0);
      const blobParts = [];
      let added = 0;

      for (const part of this[_parts]) {
        // don't add the overflow to new blobParts
        if (added >= span) {
          // Could maybe be possible to remove variable `added`
          // and only use relativeEnd?
          break;
        }
        const size = part.size;
        if (relativeStart && size <= relativeStart) {
          // Skip the beginning and change the relative
          // start & end position as we skip the unwanted parts
          relativeStart -= size;
          relativeEnd -= size;
        } else {
          const chunk = part.slice(
            relativeStart,
            MathMin(part.size, relativeEnd),
          );
          added += chunk.size;
          relativeEnd -= part.size;
          ArrayPrototypePush(blobParts, chunk);
          relativeStart = 0; // All next sequential parts should start at 0
        }
      }

      /** @type {string} */
      let relativeContentType;
      if (contentType === undefined) {
        relativeContentType = "";
      } else {
        relativeContentType = normalizeType(contentType);
      }

      const blob = new Blob([], { type: relativeContentType });
      blob[_parts] = blobParts;
      blob[_size] = span;
      return blob;
    }

    /**
     * @returns {ReadableStream<Uint8Array>}
     */
    stream() {
      webidl.assertBranded(this, BlobPrototype);
      const partIterator = toIterator(this[_parts]);
      const stream = new ReadableStream({
        type: "bytes",
        /** @param {ReadableByteStreamController} controller */
        async pull(controller) {
          while (true) {
            const { value, done } = await partIterator.next();
            if (done) return controller.close();
            if (value.byteLength > 0) {
              return controller.enqueue(value);
            }
          }
        },
      });
      return stream;
    }

    /**
     * @returns {Promise<string>}
     */
    async text() {
      webidl.assertBranded(this, BlobPrototype);
      const buffer = await this.#u8Array(this.size);
      return core.decode(buffer);
    }

    async #u8Array(size) {
      const bytes = new Uint8Array(size);
      const partIterator = toIterator(this[_parts]);
      let offset = 0;
      for await (const chunk of partIterator) {
        const byteLength = chunk.byteLength;
        if (byteLength > 0) {
          TypedArrayPrototypeSet(bytes, chunk, offset);
          offset += byteLength;
        }
      }
      return bytes;
    }

    /**
     * @returns {Promise<ArrayBuffer>}
     */
    async arrayBuffer() {
      webidl.assertBranded(this, BlobPrototype);
      const buf = await this.#u8Array(this.size);
      return buf.buffer;
    }

    [SymbolFor("Deno.customInspect")](inspect) {
      return inspect(consoleInternal.createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(BlobPrototype, this),
        keys: [
          "size",
          "type",
        ],
      }));
    }
  }

  webidl.configurePrototype(Blob);
  const BlobPrototype = Blob.prototype;

  webidl.converters["Blob"] = webidl.createInterfaceConverter(
    "Blob",
    Blob.prototype,
  );
  webidl.converters["BlobPart"] = (V, opts) => {
    // Union for ((ArrayBuffer or ArrayBufferView) or Blob or USVString)
    if (typeof V == "object") {
      if (ObjectPrototypeIsPrototypeOf(BlobPrototype, V)) {
        return webidl.converters["Blob"](V, opts);
      }
      if (
        ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, V) ||
        ObjectPrototypeIsPrototypeOf(SharedArrayBuffer.prototype, V)
      ) {
        return webidl.converters["ArrayBuffer"](V, opts);
      }
      if (ArrayBufferIsView(V)) {
        return webidl.converters["ArrayBufferView"](V, opts);
      }
    }
    // BlobPart is passed to processBlobParts after conversion, which calls core.encode()
    // on the string.
    // core.encode() is equivalent to USVString normalization.
    return webidl.converters["DOMString"](V, opts);
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
  const _LastModified = Symbol("[[LastModified]]");

  class File extends Blob {
    /** @type {string} */
    [_Name];
    /** @type {number} */
    [_LastModified];

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
        this[_LastModified] = DatePrototypeGetTime(new Date());
      } else {
        /** @type {number} */
        this[_LastModified] = options.lastModified;
      }
    }

    /** @returns {string} */
    get name() {
      webidl.assertBranded(this, FilePrototype);
      return this[_Name];
    }

    /** @returns {number} */
    get lastModified() {
      webidl.assertBranded(this, FilePrototype);
      return this[_LastModified];
    }
  }

  webidl.configurePrototype(File);
  const FilePrototype = File.prototype;

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

  // A finalization registry to deallocate a blob part when its JS reference is
  // garbage collected.
  const registry = new FinalizationRegistry((uuid) => {
    ops.op_blob_remove_part(uuid);
  });

  // TODO(lucacasonato): get a better stream from Rust in BlobReference#stream

  /**
   * An opaque reference to a blob part in Rust. This could be backed by a file,
   * in memory storage, or something else.
   */
  class BlobReference {
    /**
     * Don't use directly. Use `BlobReference.fromUint8Array`.
     * @param {string} id
     * @param {number} size
     */
    constructor(id, size) {
      this._id = id;
      this.size = size;
      registry.register(this, id);
    }

    /**
     * Create a new blob part from a Uint8Array.
     *
     * @param {Uint8Array} data
     * @returns {BlobReference}
     */
    static fromUint8Array(data) {
      const id = ops.op_blob_create_part(data);
      return new BlobReference(id, data.byteLength);
    }

    /**
     * Create a new BlobReference by slicing this BlobReference. This is a copy
     * free operation - the sliced reference will still reference the original
     * underlying bytes.
     *
     * @param {number} start
     * @param {number} end
     * @returns {BlobReference}
     */
    slice(start, end) {
      const size = end - start;
      const id = ops.op_blob_slice_part(this._id, {
        start,
        len: size,
      });
      return new BlobReference(id, size);
    }

    /**
     * Read the entire contents of the reference blob.
     * @returns {AsyncGenerator<Uint8Array>}
     */
    async *stream() {
      yield core.opAsync("op_blob_read_part", this._id);

      // let position = 0;
      // const end = this.size;
      // while (position !== end) {
      //   const size = MathMin(end - position, 65536);
      //   const chunk = this.slice(position, position + size);
      //   position += chunk.size;
      //   yield core.opAsync("op_blob_read_part", chunk._id);
      // }
    }
  }

  /**
   * Construct a new Blob object from an object URL.
   *
   * This new object will not duplicate data in memory with the original Blob
   * object from which this URL was created or with other Blob objects created
   * from the same URL, but they will be different objects.
   *
   * The object returned from this function will not be a File object, even if
   * the original object from which the object URL was constructed was one. This
   * means that the `name` and `lastModified` properties are lost.
   *
   * @param {string} url
   * @returns {Blob | null}
   */
  function blobFromObjectUrl(url) {
    const blobData = ops.op_blob_from_object_url(url);
    if (blobData === null) {
      return null;
    }

    /** @type {BlobReference[]} */
    const parts = [];
    let totalSize = 0;

    for (const { uuid, size } of blobData.parts) {
      ArrayPrototypePush(parts, new BlobReference(uuid, size));
      totalSize += size;
    }

    const blob = webidl.createBranded(Blob);
    blob[_type] = blobData.media_type;
    blob[_size] = totalSize;
    blob[_parts] = parts;
    return blob;
  }

  window.__bootstrap.file = {
    blobFromObjectUrl,
    getParts,
    Blob,
    BlobPrototype,
    File,
    FilePrototype,
  };
})(this);
