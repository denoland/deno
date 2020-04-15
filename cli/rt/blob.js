System.register(
  "$deno$/web/blob.ts",
  [
    "$deno$/web/text_encoding.ts",
    "$deno$/build.ts",
    "$deno$/web/streams/mod.ts",
  ],
  function (exports_84, context_84) {
    "use strict";
    let text_encoding_ts_6, build_ts_8, mod_ts_1, bytesSymbol, DenoBlob;
    const __moduleName = context_84 && context_84.id;
    function containsOnlyASCII(str) {
      if (typeof str !== "string") {
        return false;
      }
      return /^[\x00-\x7F]*$/.test(str);
    }
    exports_84("containsOnlyASCII", containsOnlyASCII);
    function convertLineEndingsToNative(s) {
      const nativeLineEnd = build_ts_8.build.os == "win" ? "\r\n" : "\n";
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
    function collectSequenceNotCRLF(s, position) {
      const start = position;
      for (
        let c = s.charAt(position);
        position < s.length && !(c == "\r" || c == "\n");
        c = s.charAt(++position)
      );
      return { collected: s.slice(start, position), newPosition: position };
    }
    function toUint8Arrays(blobParts, doNormalizeLineEndingsToNative) {
      const ret = [];
      const enc = new text_encoding_ts_6.TextEncoder();
      for (const element of blobParts) {
        if (typeof element === "string") {
          let str = element;
          if (doNormalizeLineEndingsToNative) {
            str = convertLineEndingsToNative(element);
          }
          ret.push(enc.encode(str));
          // eslint-disable-next-line @typescript-eslint/no-use-before-define
        } else if (element instanceof DenoBlob) {
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
    function processBlobParts(blobParts, options) {
      const normalizeLineEndingsToNative = options.ending === "native";
      // ArrayBuffer.transfer is not yet implemented in V8, so we just have to
      // pre compute size of the array buffer and do some sort of static allocation
      // instead of dynamic allocation.
      const uint8Arrays = toUint8Arrays(
        blobParts,
        normalizeLineEndingsToNative
      );
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
      return new mod_ts_1.ReadableStream({
        start: (controller) => {
          controller.enqueue(blobBytes);
          controller.close();
        },
      });
    }
    async function readBytes(reader) {
      const chunks = [];
      while (true) {
        try {
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
            return Promise.resolve(bytes);
          } else {
            return Promise.reject(new TypeError());
          }
        } catch (e) {
          return Promise.reject(e);
        }
      }
    }
    return {
      setters: [
        function (text_encoding_ts_6_1) {
          text_encoding_ts_6 = text_encoding_ts_6_1;
        },
        function (build_ts_8_1) {
          build_ts_8 = build_ts_8_1;
        },
        function (mod_ts_1_1) {
          mod_ts_1 = mod_ts_1_1;
        },
      ],
      execute: function () {
        exports_84("bytesSymbol", (bytesSymbol = Symbol("bytes")));
        // A WeakMap holding blob to byte array mapping.
        // Ensures it does not impact garbage collection.
        exports_84("blobBytesWeakMap", new WeakMap());
        DenoBlob = class DenoBlob {
          constructor(blobParts, options) {
            this.size = 0;
            this.type = "";
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
            return new DenoBlob([this[bytesSymbol].slice(start, end)], {
              type: contentType || this.type,
            });
          }
          stream() {
            return getStream(this[bytesSymbol]);
          }
          async text() {
            const reader = getStream(this[bytesSymbol]).getReader();
            const decoder = new text_encoding_ts_6.TextDecoder();
            return decoder.decode(await readBytes(reader));
          }
          arrayBuffer() {
            return readBytes(getStream(this[bytesSymbol]).getReader());
          }
        };
        exports_84("DenoBlob", DenoBlob);
      },
    };
  }
);
