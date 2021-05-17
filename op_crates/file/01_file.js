// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference no-default-lib="true" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_file.d.ts" />
/// <reference lib="esnext" />
"use strict";

((window) => {
  const webidl = window.__bootstrap.webidl;

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

  /**
   * @param  {...Uint8Array} bytesArrays
   * @returns {Uint8Array} 
   */
  function concatUint8Arrays(...bytesArrays) {
    let byteLength = 0;
    for (const bytes of bytesArrays) {
      byteLength += bytes.byteLength;
    }
    const finalBytes = new Uint8Array(byteLength);
    let current = 0;
    for (const bytes of bytesArrays) {
      finalBytes.set(bytes, current);
      current += bytes.byteLength;
    }
    return finalBytes;
  }

  const utf8Encoder = new TextEncoder();
  const utf8Decoder = new TextDecoder();

  /** @typedef {BufferSource | Blob | string} BlobPart */

  /** 
     * @param {BlobPart[]} parts
     * @param {string} endings
     * @returns {Uint8Array}
     */
  function processBlobParts(parts, endings) {
    /** @type {Uint8Array[]} */
    const bytesArrays = [];
    for (const element of parts) {
      if (element instanceof ArrayBuffer) {
        bytesArrays.push(new Uint8Array(element.slice(0)));
      } else if (ArrayBuffer.isView(element)) {
        const buffer = element.buffer.slice(
          element.byteOffset,
          element.byteOffset + element.byteLength,
        );
        bytesArrays.push(new Uint8Array(buffer));
      } else if (element instanceof Blob) {
        bytesArrays.push(
          new Uint8Array(element[_byteSequence].buffer.slice(0)),
        );
      } else if (typeof element === "string") {
        let s = element;
        if (endings == "native") {
          s = convertLineEndingsToNative(s);
        }
        bytesArrays.push(utf8Encoder.encode(s));
      } else {
        throw new TypeError("Unreachable code (invalild element type)");
      }
    }
    return concatUint8Arrays(...bytesArrays);
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

  const _byteSequence = Symbol("[[ByteSequence]]");

  class Blob {
    get [Symbol.toStringTag]() {
      return "Blob";
    }

    /** @type {string} */
    #type;

    /** @type {Uint8Array} */
    [_byteSequence];

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

      /** @type {Uint8Array} */
      this[_byteSequence] = processBlobParts(
        blobParts,
        options.endings,
      );
      this.#type = normalizeType(options.type);
    }

    /** @returns {number} */
    get size() {
      webidl.assertBranded(this, Blob);
      return this[_byteSequence].byteLength;
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
      /** @type {string} */
      let relativeContentType;
      if (contentType === undefined) {
        relativeContentType = "";
      } else {
        relativeContentType = normalizeType(contentType);
      }
      return new Blob([
        O[_byteSequence].buffer.slice(relativeStart, relativeEnd),
      ], { type: relativeContentType });
    }

    /**
     * @returns {ReadableStream<Uint8Array>}
     */
    stream() {
      webidl.assertBranded(this, Blob);
      const bytes = this[_byteSequence];
      const stream = new ReadableStream({
        type: "bytes",
        /** @param {ReadableByteStreamController} controller */
        start(controller) {
          const chunk = new Uint8Array(bytes.buffer.slice(0));
          if (chunk.byteLength > 0) controller.enqueue(chunk);
          controller.close();
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
      return utf8Decoder.decode(buffer);
    }

    /**
     * @returns {Promise<ArrayBuffer>}
     */
    async arrayBuffer() {
      webidl.assertBranded(this, Blob);
      const stream = this.stream();
      let bytes = new Uint8Array();
      for await (const chunk of stream) {
        bytes = concatUint8Arrays(bytes, chunk);
      }
      return bytes.buffer;
    }
  }

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
      this[_Name] = fileName.replaceAll("/", ":");
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

  window.__bootstrap.file = {
    Blob,
    _byteSequence,
    File,
  };
})(this);
