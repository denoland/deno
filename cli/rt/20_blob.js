// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { build } = window.__bootstrap.build;
  const { ReadableStream } = window.__bootstrap.streams;

  const bytesSymbol = Symbol("bytes");

  function containsOnlyASCII(str) {
    if (typeof str !== "string") {
      return false;
    }
    return /^[\x00-\x7F]*$/.test(str);
  }

  function convertLineEndingsToNative(s) {
    const nativeLineEnd = build.os == "windows" ? "\r\n" : "\n";

    let position = 0;

    let collectionResult = collectSequenceNotCRLF(s, position);

    let token = collectionResult.collected;
    position = collectionResult.newPosition;

    let result = token;

    while (position < s.length) {
      const c = s.charAt(position);
      if (c == "\r") {
        result += nativeLineEnd;
        position++;
        if (position < s.length && s.charAt(position) == "\n") {
          position++;
        }
      } else if (c == "\n") {
        position++;
        result += nativeLineEnd;
      }

      collectionResult = collectSequenceNotCRLF(s, position);

      token = collectionResult.collected;
      position = collectionResult.newPosition;

      result += token;
    }

    return result;
  }

  function collectSequenceNotCRLF(
    s,
    position,
  ) {
    const start = position;
    for (
      let c = s.charAt(position);
      position < s.length && !(c == "\r" || c == "\n");
      c = s.charAt(++position)
    );
    return { collected: s.slice(start, position), newPosition: position };
  }

  function toUint8Arrays(
    blobParts,
    doNormalizeLineEndingsToNative,
  ) {
    const ret = [];
    const enc = new TextEncoder();
    for (const element of blobParts) {
      if (typeof element === "string") {
        let str = element;
        if (doNormalizeLineEndingsToNative) {
          str = convertLineEndingsToNative(element);
        }
        ret.push(enc.encode(str));
        // eslint-disable-next-line @typescript-eslint/no-use-before-define
      } else if (element instanceof Blob) {
        ret.push(element[bytesSymbol]);
      } else if (element instanceof Uint8Array) {
        ret.push(element);
      } else if (element instanceof Uint16Array) {
        const uint8 = new Uint8Array(element.buffer);
        ret.push(uint8);
      } else if (element instanceof Uint32Array) {
        const uint8 = new Uint8Array(element.buffer);
        ret.push(uint8);
      } else if (ArrayBuffer.isView(element)) {
        // Convert view to Uint8Array.
        const uint8 = new Uint8Array(element.buffer);
        ret.push(uint8);
      } else if (element instanceof ArrayBuffer) {
        // Create a new Uint8Array view for the given ArrayBuffer.
        const uint8 = new Uint8Array(element);
        ret.push(uint8);
      } else {
        ret.push(enc.encode(String(element)));
      }
    }
    return ret;
  }

  function processBlobParts(
    blobParts,
    options,
  ) {
    const normalizeLineEndingsToNative = options.ending === "native";
    // ArrayBuffer.transfer is not yet implemented in V8, so we just have to
    // pre compute size of the array buffer and do some sort of static allocation
    // instead of dynamic allocation.
    const uint8Arrays = toUint8Arrays(blobParts, normalizeLineEndingsToNative);
    const byteLength = uint8Arrays
      .map((u8) => u8.byteLength)
      .reduce((a, b) => a + b, 0);
    const ab = new ArrayBuffer(byteLength);
    const bytes = new Uint8Array(ab);
    let courser = 0;
    for (const u8 of uint8Arrays) {
      bytes.set(u8, courser);
      courser += u8.byteLength;
    }

    return bytes;
  }

  function getStream(blobBytes) {
    // TODO: Align to spec https://fetch.spec.whatwg.org/#concept-construct-readablestream
    return new ReadableStream({
      type: "bytes",
      start: (controller) => {
        controller.enqueue(blobBytes);
        controller.close();
      },
    });
  }

  async function readBytes(
    reader,
  ) {
    const chunks = [];
    while (true) {
      const { done, value } = await reader.read();
      if (!done && value instanceof Uint8Array) {
        chunks.push(value);
      } else if (done) {
        const size = chunks.reduce((p, i) => p + i.byteLength, 0);
        const bytes = new Uint8Array(size);
        let offs = 0;
        for (const chunk of chunks) {
          bytes.set(chunk, offs);
          offs += chunk.byteLength;
        }
        return bytes.buffer;
      } else {
        throw new TypeError("Invalid reader result.");
      }
    }
  }

  // A WeakMap holding blob to byte array mapping.
  // Ensures it does not impact garbage collection.
  const blobBytesWeakMap = new WeakMap();

  class Blob {
    constructor(blobParts, options) {
      if (arguments.length === 0) {
        this[bytesSymbol] = new Uint8Array();
        return;
      }

      const { ending = "transparent", type = "" } = options ?? {};
      // Normalize options.type.
      let normalizedType = type;
      if (!containsOnlyASCII(type)) {
        normalizedType = "";
      } else {
        if (type.length) {
          for (let i = 0; i < type.length; ++i) {
            const char = type[i];
            if (char < "\u0020" || char > "\u007E") {
              normalizedType = "";
              break;
            }
          }
          normalizedType = type.toLowerCase();
        }
      }
      const bytes = processBlobParts(blobParts, { ending, type });
      // Set Blob object's properties.
      this[bytesSymbol] = bytes;
      this.size = bytes.byteLength;
      this.type = normalizedType;
    }

    slice(start, end, contentType) {
      return new Blob([this[bytesSymbol].slice(start, end)], {
        type: contentType || this.type,
      });
    }

    stream() {
      return getStream(this[bytesSymbol]);
    }

    async text() {
      const reader = getStream(this[bytesSymbol]).getReader();
      const decoder = new TextDecoder();
      return decoder.decode(await readBytes(reader));
    }

    arrayBuffer() {
      return readBytes(getStream(this[bytesSymbol]).getReader());
    }
  }

  window.__bootstrap.blob = {
    Blob,
    bytesSymbol,
    containsOnlyASCII,
    blobBytesWeakMap,
  };
})(this);
