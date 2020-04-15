// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// The following code is based off of text-encoding at:
// https://github.com/inexorabletash/text-encoding
//
// Anyone is free to copy, modify, publish, use, compile, sell, or
// distribute this software, either in source code form or as a compiled
// binary, for any purpose, commercial or non-commercial, and by any
// means.
//
// In jurisdictions that recognize copyright laws, the author or authors
// of this software dedicate any and all copyright interest in the
// software to the public domain. We make this dedication for the benefit
// of the public at large and to the detriment of our heirs and
// successors. We intend this dedication to be an overt act of
// relinquishment in perpetuity of all present and future rights to this
// software under copyright law.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY CLAIM, DAMAGES OR
// OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
System.register(
  "$deno$/web/text_encoding.ts",
  ["$deno$/web/base64.ts", "$deno$/web/decode_utf8.ts", "$deno$/core.ts"],
  function (exports_6, context_6) {
    "use strict";
    let base64,
      decode_utf8_ts_1,
      core_ts_1,
      CONTINUE,
      END_OF_STREAM,
      FINISHED,
      UTF8Encoder,
      SingleByteDecoder,
      encodingMap,
      encodings,
      decoders,
      encodingIndexes,
      Stream,
      TextDecoder,
      TextEncoder;
    const __moduleName = context_6 && context_6.id;
    function decoderError(fatal) {
      if (fatal) {
        throw new TypeError("Decoder error.");
      }
      return 0xfffd; // default code point
    }
    function inRange(a, min, max) {
      return min <= a && a <= max;
    }
    function isASCIIByte(a) {
      return inRange(a, 0x00, 0x7f);
    }
    function stringToCodePoints(input) {
      const u = [];
      for (const c of input) {
        u.push(c.codePointAt(0));
      }
      return u;
    }
    function atob(s) {
      s = String(s);
      s = s.replace(/[\t\n\f\r ]/g, "");
      if (s.length % 4 === 0) {
        s = s.replace(/==?$/, "");
      }
      const rem = s.length % 4;
      if (rem === 1 || /[^+/0-9A-Za-z]/.test(s)) {
        // TODO: throw `DOMException`
        throw new TypeError(
          "The string to be decoded is not correctly encoded"
        );
      }
      // base64-js requires length exactly times of 4
      if (rem > 0) {
        s = s.padEnd(s.length + (4 - rem), "=");
      }
      const byteArray = base64.toByteArray(s);
      let result = "";
      for (let i = 0; i < byteArray.length; i++) {
        result += String.fromCharCode(byteArray[i]);
      }
      return result;
    }
    exports_6("atob", atob);
    function btoa(s) {
      const byteArray = [];
      for (let i = 0; i < s.length; i++) {
        const charCode = s[i].charCodeAt(0);
        if (charCode > 0xff) {
          throw new TypeError(
            "The string to be encoded contains characters " +
              "outside of the Latin1 range."
          );
        }
        byteArray.push(charCode);
      }
      const result = base64.fromByteArray(Uint8Array.from(byteArray));
      return result;
    }
    exports_6("btoa", btoa);
    function codePointsToString(codePoints) {
      let s = "";
      for (const cp of codePoints) {
        s += String.fromCodePoint(cp);
      }
      return s;
    }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    function isEitherArrayBuffer(x) {
      return x instanceof SharedArrayBuffer || x instanceof ArrayBuffer;
    }
    return {
      setters: [
        function (base64_1) {
          base64 = base64_1;
        },
        function (decode_utf8_ts_1_1) {
          decode_utf8_ts_1 = decode_utf8_ts_1_1;
        },
        function (core_ts_1_1) {
          core_ts_1 = core_ts_1_1;
        },
      ],
      execute: function () {
        CONTINUE = null;
        END_OF_STREAM = -1;
        FINISHED = -1;
        UTF8Encoder = class UTF8Encoder {
          handler(codePoint) {
            if (codePoint === END_OF_STREAM) {
              return FINISHED;
            }
            if (inRange(codePoint, 0x00, 0x7f)) {
              return codePoint;
            }
            let count;
            let offset;
            if (inRange(codePoint, 0x0080, 0x07ff)) {
              count = 1;
              offset = 0xc0;
            } else if (inRange(codePoint, 0x0800, 0xffff)) {
              count = 2;
              offset = 0xe0;
            } else if (inRange(codePoint, 0x10000, 0x10ffff)) {
              count = 3;
              offset = 0xf0;
            } else {
              throw TypeError(
                `Code point out of range: \\x${codePoint.toString(16)}`
              );
            }
            const bytes = [(codePoint >> (6 * count)) + offset];
            while (count > 0) {
              const temp = codePoint >> (6 * (count - 1));
              bytes.push(0x80 | (temp & 0x3f));
              count--;
            }
            return bytes;
          }
        };
        SingleByteDecoder = class SingleByteDecoder {
          constructor(index, { ignoreBOM = false, fatal = false } = {}) {
            if (ignoreBOM) {
              throw new TypeError(
                "Ignoring the BOM is available only with utf-8."
              );
            }
            this.#fatal = fatal;
            this.#index = index;
          }
          #index;
          #fatal;
          handler(_stream, byte) {
            if (byte === END_OF_STREAM) {
              return FINISHED;
            }
            if (isASCIIByte(byte)) {
              return byte;
            }
            const codePoint = this.#index[byte - 0x80];
            if (codePoint == null) {
              return decoderError(this.#fatal);
            }
            return codePoint;
          }
        };
        // The encodingMap is a hash of labels that are indexed by the conical
        // encoding.
        encodingMap = {
          "windows-1252": [
            "ansi_x3.4-1968",
            "ascii",
            "cp1252",
            "cp819",
            "csisolatin1",
            "ibm819",
            "iso-8859-1",
            "iso-ir-100",
            "iso8859-1",
            "iso88591",
            "iso_8859-1",
            "iso_8859-1:1987",
            "l1",
            "latin1",
            "us-ascii",
            "windows-1252",
            "x-cp1252",
          ],
          "utf-8": ["unicode-1-1-utf-8", "utf-8", "utf8"],
        };
        // We convert these into a Map where every label resolves to its canonical
        // encoding type.
        encodings = new Map();
        for (const key of Object.keys(encodingMap)) {
          const labels = encodingMap[key];
          for (const label of labels) {
            encodings.set(label, key);
          }
        }
        // A map of functions that return new instances of a decoder indexed by the
        // encoding type.
        decoders = new Map();
        // Single byte decoders are an array of code point lookups
        encodingIndexes = new Map();
        // prettier-ignore
        encodingIndexes.set("windows-1252", [
                8364,
                129,
                8218,
                402,
                8222,
                8230,
                8224,
                8225,
                710,
                8240,
                352,
                8249,
                338,
                141,
                381,
                143,
                144,
                8216,
                8217,
                8220,
                8221,
                8226,
                8211,
                8212,
                732,
                8482,
                353,
                8250,
                339,
                157,
                382,
                376,
                160,
                161,
                162,
                163,
                164,
                165,
                166,
                167,
                168,
                169,
                170,
                171,
                172,
                173,
                174,
                175,
                176,
                177,
                178,
                179,
                180,
                181,
                182,
                183,
                184,
                185,
                186,
                187,
                188,
                189,
                190,
                191,
                192,
                193,
                194,
                195,
                196,
                197,
                198,
                199,
                200,
                201,
                202,
                203,
                204,
                205,
                206,
                207,
                208,
                209,
                210,
                211,
                212,
                213,
                214,
                215,
                216,
                217,
                218,
                219,
                220,
                221,
                222,
                223,
                224,
                225,
                226,
                227,
                228,
                229,
                230,
                231,
                232,
                233,
                234,
                235,
                236,
                237,
                238,
                239,
                240,
                241,
                242,
                243,
                244,
                245,
                246,
                247,
                248,
                249,
                250,
                251,
                252,
                253,
                254,
                255,
            ]);
        for (const [key, index] of encodingIndexes) {
          decoders.set(key, (options) => {
            return new SingleByteDecoder(index, options);
          });
        }
        Stream = class Stream {
          constructor(tokens) {
            this.#tokens = [...tokens];
            this.#tokens.reverse();
          }
          #tokens;
          endOfStream() {
            return !this.#tokens.length;
          }
          read() {
            return !this.#tokens.length ? END_OF_STREAM : this.#tokens.pop();
          }
          prepend(token) {
            if (Array.isArray(token)) {
              while (token.length) {
                this.#tokens.push(token.pop());
              }
            } else {
              this.#tokens.push(token);
            }
          }
          push(token) {
            if (Array.isArray(token)) {
              while (token.length) {
                this.#tokens.unshift(token.shift());
              }
            } else {
              this.#tokens.unshift(token);
            }
          }
        };
        TextDecoder = class TextDecoder {
          constructor(label = "utf-8", options = { fatal: false }) {
            this.fatal = false;
            this.ignoreBOM = false;
            if (options.ignoreBOM) {
              this.ignoreBOM = true;
            }
            if (options.fatal) {
              this.fatal = true;
            }
            label = String(label).trim().toLowerCase();
            const encoding = encodings.get(label);
            if (!encoding) {
              throw new RangeError(
                `The encoding label provided ('${label}') is invalid.`
              );
            }
            if (!decoders.has(encoding) && encoding !== "utf-8") {
              throw new TypeError(
                `Internal decoder ('${encoding}') not found.`
              );
            }
            this.#encoding = encoding;
          }
          #encoding;
          get encoding() {
            return this.#encoding;
          }
          decode(input, options = { stream: false }) {
            if (options.stream) {
              throw new TypeError("Stream not supported.");
            }
            let bytes;
            if (input instanceof Uint8Array) {
              bytes = input;
            } else if (isEitherArrayBuffer(input)) {
              bytes = new Uint8Array(input);
            } else if (
              typeof input === "object" &&
              "buffer" in input &&
              isEitherArrayBuffer(input.buffer)
            ) {
              bytes = new Uint8Array(
                input.buffer,
                input.byteOffset,
                input.byteLength
              );
            } else {
              bytes = new Uint8Array(0);
            }
            // For simple utf-8 decoding "Deno.core.decode" can be used for performance
            if (
              this.#encoding === "utf-8" &&
              this.fatal === false &&
              this.ignoreBOM === false
            ) {
              return core_ts_1.core.decode(bytes);
            }
            // For performance reasons we utilise a highly optimised decoder instead of
            // the general decoder.
            if (this.#encoding === "utf-8") {
              return decode_utf8_ts_1.decodeUtf8(
                bytes,
                this.fatal,
                this.ignoreBOM
              );
            }
            const decoder = decoders.get(this.#encoding)({
              fatal: this.fatal,
              ignoreBOM: this.ignoreBOM,
            });
            const inputStream = new Stream(bytes);
            const output = [];
            while (true) {
              const result = decoder.handler(inputStream, inputStream.read());
              if (result === FINISHED) {
                break;
              }
              if (result !== CONTINUE) {
                output.push(result);
              }
            }
            if (output.length > 0 && output[0] === 0xfeff) {
              output.shift();
            }
            return codePointsToString(output);
          }
          get [Symbol.toStringTag]() {
            return "TextDecoder";
          }
        };
        exports_6("TextDecoder", TextDecoder);
        TextEncoder = class TextEncoder {
          constructor() {
            this.encoding = "utf-8";
          }
          encode(input = "") {
            // Deno.core.encode() provides very efficient utf-8 encoding
            if (this.encoding === "utf-8") {
              return core_ts_1.core.encode(input);
            }
            const encoder = new UTF8Encoder();
            const inputStream = new Stream(stringToCodePoints(input));
            const output = [];
            while (true) {
              const result = encoder.handler(inputStream.read());
              if (result === FINISHED) {
                break;
              }
              if (Array.isArray(result)) {
                output.push(...result);
              } else {
                output.push(result);
              }
            }
            return new Uint8Array(output);
          }
          encodeInto(input, dest) {
            const encoder = new UTF8Encoder();
            const inputStream = new Stream(stringToCodePoints(input));
            let written = 0;
            let read = 0;
            while (true) {
              const result = encoder.handler(inputStream.read());
              if (result === FINISHED) {
                break;
              }
              read++;
              if (Array.isArray(result)) {
                dest.set(result, written);
                written += result.length;
                if (result.length > 3) {
                  // increment read a second time if greater than U+FFFF
                  read++;
                }
              } else {
                dest[written] = result;
                written++;
              }
            }
            return {
              read,
              written,
            };
          }
          get [Symbol.toStringTag]() {
            return "TextEncoder";
          }
        };
        exports_6("TextEncoder", TextEncoder);
      },
    };
  }
);
