// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference no-default-lib="true" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_fetch.d.ts" />
/// <reference lib="esnext" />

((window) => {
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
    /** @type {string} */
    #type;

    /** @type {Uint8Array} */
    [_byteSequence];

    /**
     * @param {BlobPart[]} [blobParts]
     * @param {BlobPropertyBag} [options]
     */
    constructor(blobParts, options) {
      if (blobParts === undefined) {
        blobParts = [];
      }
      if (typeof blobParts !== "object") {
        throw new TypeError(
          `Failed to construct 'Blob'. blobParts cannot be converted to a sequence.`,
        );
      }

      const parts = [];
      const iterator = blobParts[Symbol.iterator]?.();
      if (iterator === undefined) {
        throw new TypeError(
          "Failed to construct 'Blob'. The provided value cannot be converted to a sequence",
        );
      }
      while (true) {
        const { value: element, done } = iterator.next();
        if (done) break;
        if (
          ArrayBuffer.isView(element) || element instanceof ArrayBuffer ||
          element instanceof Blob
        ) {
          parts.push(element);
        } else {
          parts.push(String(element));
        }
      }

      if (!options || typeof options === "function") {
        options = {};
      }
      if (typeof options !== "object") {
        throw new TypeError(
          `Failed to construct 'Blob'. options is not an object.`,
        );
      }
      const endings = options.endings?.toString() ?? "transparent";
      const type = options.type?.toString() ?? "";

      /** @type {Uint8Array} */
      this[_byteSequence] = processBlobParts(parts, endings);
      this.#type = normalizeType(type);
    }

    /** @returns {number} */
    get size() {
      return this[_byteSequence].byteLength;
    }

    /** @returns {string} */
    get type() {
      return this.#type;
    }

    /** 
     * @param {number} [start]
     * @param {number} [end]
     * @param {string} [contentType]
     * @returns {Blob}
     */
    slice(start, end, contentType) {
      const O = this;
      /** @type {number} */
      let relativeStart;
      if (start === undefined) {
        relativeStart = 0;
      } else {
        start = Number(start);
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
        end = Number(end);
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
        relativeContentType = normalizeType(String(contentType));
      }
      return new Blob([
        O[_byteSequence].buffer.slice(relativeStart, relativeEnd),
      ], { type: relativeContentType });
    }

    /**
     * @returns {ReadableStream<Uint8Array>}
     */
    stream() {
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
      const buffer = await this.arrayBuffer();
      return utf8Decoder.decode(buffer);
    }

    /**
     * @returns {Promise<ArrayBuffer>}
     */
    async arrayBuffer() {
      const stream = this.stream();
      let bytes = new Uint8Array();
      for await (const chunk of stream) {
        bytes = concatUint8Arrays(bytes, chunk);
      }
      return bytes.buffer;
    }

    get [Symbol.toStringTag]() {
      return "Blob";
    }
  }

  const _Name = Symbol("[[Name]]");
  const _LastModfied = Symbol("[[LastModified]]");

  class File extends Blob {
    /** @type {string} */
    [_Name];
    /** @type {number} */
    [_LastModfied];

    /**
     * @param {BlobPart[]} fileBits 
     * @param {string} fileName 
     * @param {FilePropertyBag} [options] 
     */
    constructor(fileBits, fileName, options) {
      if (fileBits === undefined) {
        throw new TypeError(
          "Failed to construct 'File'. 2 arguments required, but first not specified.",
        );
      }
      if (fileName === undefined) {
        throw new TypeError(
          "Failed to construct 'File'. 2 arguments required, but second not specified.",
        );
      }
      super(fileBits, { endings: options?.endings, type: options?.type });
      /** @type {string} */
      this[_Name] = String(fileName).replaceAll("/", ":");
      if (options?.lastModified === undefined) {
        /** @type {number} */
        this[_LastModfied] = new Date().getTime();
      } else {
        /** @type {number} */
        this[_LastModfied] = Number(options.lastModified);
      }
    }

    /** @returns {string} */
    get name() {
      return this[_Name];
    }

    /** @returns {number} */
    get lastModified() {
      return this[_LastModfied];
    }
  }

  window.__bootstrap.file = {
    Blob,
    _byteSequence,
    File,
  };
})(this);
