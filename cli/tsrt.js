// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

System.register("$deno$/diagnostics.ts", [], function (exports_1, context_1) {
  "use strict";
  let DiagnosticCategory;
  const __moduleName = context_1 && context_1.id;
  return {
    setters: [],
    execute: function () {
      // Diagnostic provides an abstraction for advice/errors received from a
      // compiler, which is strongly influenced by the format of TypeScript
      // diagnostics.
      (function (DiagnosticCategory) {
        DiagnosticCategory[(DiagnosticCategory["Log"] = 0)] = "Log";
        DiagnosticCategory[(DiagnosticCategory["Debug"] = 1)] = "Debug";
        DiagnosticCategory[(DiagnosticCategory["Info"] = 2)] = "Info";
        DiagnosticCategory[(DiagnosticCategory["Error"] = 3)] = "Error";
        DiagnosticCategory[(DiagnosticCategory["Warning"] = 4)] = "Warning";
        DiagnosticCategory[(DiagnosticCategory["Suggestion"] = 5)] =
          "Suggestion";
      })(DiagnosticCategory || (DiagnosticCategory = {}));
      exports_1("DiagnosticCategory", DiagnosticCategory);
    },
  };
});
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/util.ts", [], function (exports_2, context_2) {
  "use strict";
  let logDebug, logSource;
  const __moduleName = context_2 && context_2.id;
  // @internal
  function setLogDebug(debug, source) {
    logDebug = debug;
    if (source) {
      logSource = source;
    }
  }
  exports_2("setLogDebug", setLogDebug);
  function log(...args) {
    if (logDebug) {
      // if we destructure `console` off `globalThis` too early, we don't bind to
      // the right console, therefore we don't log anything out.
      globalThis.console.log(`DEBUG ${logSource} -`, ...args);
    }
  }
  exports_2("log", log);
  // @internal
  function assert(cond, msg = "assert") {
    if (!cond) {
      throw Error(msg);
    }
  }
  exports_2("assert", assert);
  // @internal
  function createResolvable() {
    let resolve;
    let reject;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    promise.resolve = resolve;
    promise.reject = reject;
    return promise;
  }
  exports_2("createResolvable", createResolvable);
  // @internal
  function notImplemented() {
    throw new Error("not implemented");
  }
  exports_2("notImplemented", notImplemented);
  // @internal
  function immutableDefine(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    o,
    p,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    value
  ) {
    Object.defineProperty(o, p, {
      value,
      configurable: false,
      writable: false,
    });
  }
  exports_2("immutableDefine", immutableDefine);
  return {
    setters: [],
    execute: function () {
      logDebug = false;
      logSource = "JS";
    },
  };
});
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/core.ts", [], function (exports_3, context_3) {
  "use strict";
  const __moduleName = context_3 && context_3.id;
  return {
    setters: [],
    execute: function () {
      // This allows us to access core in API even if we
      // dispose window.Deno
      exports_3("core", globalThis.Deno.core);
    },
  };
});
// Forked from https://github.com/beatgammit/base64-js
// Copyright (c) 2014 Jameson Little. MIT License.
System.register("$deno$/web/base64.ts", [], function (exports_4, context_4) {
  "use strict";
  let lookup, revLookup, code;
  const __moduleName = context_4 && context_4.id;
  function getLens(b64) {
    const len = b64.length;
    if (len % 4 > 0) {
      throw new Error("Invalid string. Length must be a multiple of 4");
    }
    // Trim off extra bytes after placeholder bytes are found
    // See: https://github.com/beatgammit/base64-js/issues/42
    let validLen = b64.indexOf("=");
    if (validLen === -1) validLen = len;
    const placeHoldersLen = validLen === len ? 0 : 4 - (validLen % 4);
    return [validLen, placeHoldersLen];
  }
  // base64 is 4/3 + up to two characters of the original data
  function byteLength(b64) {
    const lens = getLens(b64);
    const validLen = lens[0];
    const placeHoldersLen = lens[1];
    return ((validLen + placeHoldersLen) * 3) / 4 - placeHoldersLen;
  }
  exports_4("byteLength", byteLength);
  function _byteLength(b64, validLen, placeHoldersLen) {
    return ((validLen + placeHoldersLen) * 3) / 4 - placeHoldersLen;
  }
  function toByteArray(b64) {
    let tmp;
    const lens = getLens(b64);
    const validLen = lens[0];
    const placeHoldersLen = lens[1];
    const arr = new Uint8Array(_byteLength(b64, validLen, placeHoldersLen));
    let curByte = 0;
    // if there are placeholders, only get up to the last complete 4 chars
    const len = placeHoldersLen > 0 ? validLen - 4 : validLen;
    let i;
    for (i = 0; i < len; i += 4) {
      tmp =
        (revLookup[b64.charCodeAt(i)] << 18) |
        (revLookup[b64.charCodeAt(i + 1)] << 12) |
        (revLookup[b64.charCodeAt(i + 2)] << 6) |
        revLookup[b64.charCodeAt(i + 3)];
      arr[curByte++] = (tmp >> 16) & 0xff;
      arr[curByte++] = (tmp >> 8) & 0xff;
      arr[curByte++] = tmp & 0xff;
    }
    if (placeHoldersLen === 2) {
      tmp =
        (revLookup[b64.charCodeAt(i)] << 2) |
        (revLookup[b64.charCodeAt(i + 1)] >> 4);
      arr[curByte++] = tmp & 0xff;
    }
    if (placeHoldersLen === 1) {
      tmp =
        (revLookup[b64.charCodeAt(i)] << 10) |
        (revLookup[b64.charCodeAt(i + 1)] << 4) |
        (revLookup[b64.charCodeAt(i + 2)] >> 2);
      arr[curByte++] = (tmp >> 8) & 0xff;
      arr[curByte++] = tmp & 0xff;
    }
    return arr;
  }
  exports_4("toByteArray", toByteArray);
  function tripletToBase64(num) {
    return (
      lookup[(num >> 18) & 0x3f] +
      lookup[(num >> 12) & 0x3f] +
      lookup[(num >> 6) & 0x3f] +
      lookup[num & 0x3f]
    );
  }
  function encodeChunk(uint8, start, end) {
    let tmp;
    const output = [];
    for (let i = start; i < end; i += 3) {
      tmp =
        ((uint8[i] << 16) & 0xff0000) +
        ((uint8[i + 1] << 8) & 0xff00) +
        (uint8[i + 2] & 0xff);
      output.push(tripletToBase64(tmp));
    }
    return output.join("");
  }
  function fromByteArray(uint8) {
    let tmp;
    const len = uint8.length;
    const extraBytes = len % 3; // if we have 1 byte left, pad 2 bytes
    const parts = [];
    const maxChunkLength = 16383; // must be multiple of 3
    // go through the array every three bytes, we'll deal with trailing stuff later
    for (let i = 0, len2 = len - extraBytes; i < len2; i += maxChunkLength) {
      parts.push(
        encodeChunk(
          uint8,
          i,
          i + maxChunkLength > len2 ? len2 : i + maxChunkLength
        )
      );
    }
    // pad the end with zeros, but make sure to not forget the extra bytes
    if (extraBytes === 1) {
      tmp = uint8[len - 1];
      parts.push(lookup[tmp >> 2] + lookup[(tmp << 4) & 0x3f] + "==");
    } else if (extraBytes === 2) {
      tmp = (uint8[len - 2] << 8) + uint8[len - 1];
      parts.push(
        lookup[tmp >> 10] +
          lookup[(tmp >> 4) & 0x3f] +
          lookup[(tmp << 2) & 0x3f] +
          "="
      );
    }
    return parts.join("");
  }
  exports_4("fromByteArray", fromByteArray);
  return {
    setters: [],
    execute: function () {
      lookup = [];
      revLookup = [];
      code = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
      for (let i = 0, len = code.length; i < len; ++i) {
        lookup[i] = code[i];
        revLookup[code.charCodeAt(i)] = i;
      }
      // Support decoding URL-safe base64 strings, as Node.js does.
      // See: https://en.wikipedia.org/wiki/Base64#URL_applications
      revLookup["-".charCodeAt(0)] = 62;
      revLookup["_".charCodeAt(0)] = 63;
    },
  };
});
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// The following code is based off:
// https://github.com/inexorabletash/text-encoding
//
// Copyright (c) 2008-2009 Bjoern Hoehrmann <bjoern@hoehrmann.de>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
System.register("$deno$/web/decode_utf8.ts", [], function (
  exports_5,
  context_5
) {
  "use strict";
  const __moduleName = context_5 && context_5.id;
  function decodeUtf8(input, fatal, ignoreBOM) {
    let outString = "";
    // Prepare a buffer so that we don't have to do a lot of string concats, which
    // are very slow.
    const outBufferLength = Math.min(1024, input.length);
    const outBuffer = new Uint16Array(outBufferLength);
    let outIndex = 0;
    let state = 0;
    let codepoint = 0;
    let type;
    let i =
      ignoreBOM && input[0] === 0xef && input[1] === 0xbb && input[2] === 0xbf
        ? 3
        : 0;
    for (; i < input.length; ++i) {
      // Encoding error handling
      if (state === 12 || (state !== 0 && (input[i] & 0xc0) !== 0x80)) {
        if (fatal)
          throw new TypeError(
            `Decoder error. Invalid byte in sequence at position ${i} in data.`
          );
        outBuffer[outIndex++] = 0xfffd; // Replacement character
        if (outIndex === outBufferLength) {
          outString += String.fromCharCode.apply(null, outBuffer);
          outIndex = 0;
        }
        state = 0;
      }
      // prettier-ignore
      type = [
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9,
                7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
                8, 8, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
                10, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 3, 3, 11, 6, 6, 6, 5, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8
            ][input[i]];
      codepoint =
        state !== 0
          ? (input[i] & 0x3f) | (codepoint << 6)
          : (0xff >> type) & input[i];
      // prettier-ignore
      state = [
                0, 12, 24, 36, 60, 96, 84, 12, 12, 12, 48, 72, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12,
                12, 0, 12, 12, 12, 12, 12, 0, 12, 0, 12, 12, 12, 24, 12, 12, 12, 12, 12, 24, 12, 24, 12, 12,
                12, 12, 12, 12, 12, 12, 12, 24, 12, 12, 12, 12, 12, 24, 12, 12, 12, 12, 12, 12, 12, 24, 12, 12,
                12, 12, 12, 12, 12, 12, 12, 36, 12, 36, 12, 12, 12, 36, 12, 12, 12, 12, 12, 36, 12, 36, 12, 12,
                12, 36, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12
            ][state + type];
      if (state !== 0) continue;
      // Add codepoint to buffer (as charcodes for utf-16), and flush buffer to
      // string if needed.
      if (codepoint > 0xffff) {
        outBuffer[outIndex++] = 0xd7c0 + (codepoint >> 10);
        if (outIndex === outBufferLength) {
          outString += String.fromCharCode.apply(null, outBuffer);
          outIndex = 0;
        }
        outBuffer[outIndex++] = 0xdc00 | (codepoint & 0x3ff);
        if (outIndex === outBufferLength) {
          outString += String.fromCharCode.apply(null, outBuffer);
          outIndex = 0;
        }
      } else {
        outBuffer[outIndex++] = codepoint;
        if (outIndex === outBufferLength) {
          outString += String.fromCharCode.apply(null, outBuffer);
          outIndex = 0;
        }
      }
    }
    // Add a replacement character if we ended in the middle of a sequence or
    // encountered an invalid code at the end.
    if (state !== 0) {
      if (fatal) throw new TypeError(`Decoder error. Unexpected end of data.`);
      outBuffer[outIndex++] = 0xfffd; // Replacement character
    }
    // Final flush of buffer
    outString += String.fromCharCode.apply(
      null,
      outBuffer.subarray(0, outIndex)
    );
    return outString;
  }
  exports_5("decodeUtf8", decodeUtf8);
  return {
    setters: [],
    execute: function () {},
  };
});
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
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/errors.ts", [], function (exports_7, context_7) {
  "use strict";
  let ErrorKind,
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    BrokenPipe,
    AlreadyExists,
    InvalidData,
    TimedOut,
    Interrupted,
    WriteZero,
    UnexpectedEof,
    BadResource,
    Http;
  const __moduleName = context_7 && context_7.id;
  function getErrorClass(kind) {
    switch (kind) {
      case ErrorKind.TypeError:
        return TypeError;
      case ErrorKind.Other:
        return Error;
      case ErrorKind.URIError:
        return URIError;
      case ErrorKind.NotFound:
        return NotFound;
      case ErrorKind.PermissionDenied:
        return PermissionDenied;
      case ErrorKind.ConnectionRefused:
        return ConnectionRefused;
      case ErrorKind.ConnectionReset:
        return ConnectionReset;
      case ErrorKind.ConnectionAborted:
        return ConnectionAborted;
      case ErrorKind.NotConnected:
        return NotConnected;
      case ErrorKind.AddrInUse:
        return AddrInUse;
      case ErrorKind.AddrNotAvailable:
        return AddrNotAvailable;
      case ErrorKind.BrokenPipe:
        return BrokenPipe;
      case ErrorKind.AlreadyExists:
        return AlreadyExists;
      case ErrorKind.InvalidData:
        return InvalidData;
      case ErrorKind.TimedOut:
        return TimedOut;
      case ErrorKind.Interrupted:
        return Interrupted;
      case ErrorKind.WriteZero:
        return WriteZero;
      case ErrorKind.UnexpectedEof:
        return UnexpectedEof;
      case ErrorKind.BadResource:
        return BadResource;
      case ErrorKind.Http:
        return Http;
    }
  }
  exports_7("getErrorClass", getErrorClass);
  return {
    setters: [],
    execute: function () {
      // Warning! The values in this enum are duplicated in cli/op_error.rs
      // Update carefully!
      (function (ErrorKind) {
        ErrorKind[(ErrorKind["NotFound"] = 1)] = "NotFound";
        ErrorKind[(ErrorKind["PermissionDenied"] = 2)] = "PermissionDenied";
        ErrorKind[(ErrorKind["ConnectionRefused"] = 3)] = "ConnectionRefused";
        ErrorKind[(ErrorKind["ConnectionReset"] = 4)] = "ConnectionReset";
        ErrorKind[(ErrorKind["ConnectionAborted"] = 5)] = "ConnectionAborted";
        ErrorKind[(ErrorKind["NotConnected"] = 6)] = "NotConnected";
        ErrorKind[(ErrorKind["AddrInUse"] = 7)] = "AddrInUse";
        ErrorKind[(ErrorKind["AddrNotAvailable"] = 8)] = "AddrNotAvailable";
        ErrorKind[(ErrorKind["BrokenPipe"] = 9)] = "BrokenPipe";
        ErrorKind[(ErrorKind["AlreadyExists"] = 10)] = "AlreadyExists";
        ErrorKind[(ErrorKind["InvalidData"] = 13)] = "InvalidData";
        ErrorKind[(ErrorKind["TimedOut"] = 14)] = "TimedOut";
        ErrorKind[(ErrorKind["Interrupted"] = 15)] = "Interrupted";
        ErrorKind[(ErrorKind["WriteZero"] = 16)] = "WriteZero";
        ErrorKind[(ErrorKind["UnexpectedEof"] = 17)] = "UnexpectedEof";
        ErrorKind[(ErrorKind["BadResource"] = 18)] = "BadResource";
        ErrorKind[(ErrorKind["Http"] = 19)] = "Http";
        ErrorKind[(ErrorKind["URIError"] = 20)] = "URIError";
        ErrorKind[(ErrorKind["TypeError"] = 21)] = "TypeError";
        ErrorKind[(ErrorKind["Other"] = 22)] = "Other";
      })(ErrorKind || (ErrorKind = {}));
      exports_7("ErrorKind", ErrorKind);
      NotFound = class NotFound extends Error {
        constructor(msg) {
          super(msg);
          this.name = "NotFound";
        }
      };
      PermissionDenied = class PermissionDenied extends Error {
        constructor(msg) {
          super(msg);
          this.name = "PermissionDenied";
        }
      };
      ConnectionRefused = class ConnectionRefused extends Error {
        constructor(msg) {
          super(msg);
          this.name = "ConnectionRefused";
        }
      };
      ConnectionReset = class ConnectionReset extends Error {
        constructor(msg) {
          super(msg);
          this.name = "ConnectionReset";
        }
      };
      ConnectionAborted = class ConnectionAborted extends Error {
        constructor(msg) {
          super(msg);
          this.name = "ConnectionAborted";
        }
      };
      NotConnected = class NotConnected extends Error {
        constructor(msg) {
          super(msg);
          this.name = "NotConnected";
        }
      };
      AddrInUse = class AddrInUse extends Error {
        constructor(msg) {
          super(msg);
          this.name = "AddrInUse";
        }
      };
      AddrNotAvailable = class AddrNotAvailable extends Error {
        constructor(msg) {
          super(msg);
          this.name = "AddrNotAvailable";
        }
      };
      BrokenPipe = class BrokenPipe extends Error {
        constructor(msg) {
          super(msg);
          this.name = "BrokenPipe";
        }
      };
      AlreadyExists = class AlreadyExists extends Error {
        constructor(msg) {
          super(msg);
          this.name = "AlreadyExists";
        }
      };
      InvalidData = class InvalidData extends Error {
        constructor(msg) {
          super(msg);
          this.name = "InvalidData";
        }
      };
      TimedOut = class TimedOut extends Error {
        constructor(msg) {
          super(msg);
          this.name = "TimedOut";
        }
      };
      Interrupted = class Interrupted extends Error {
        constructor(msg) {
          super(msg);
          this.name = "Interrupted";
        }
      };
      WriteZero = class WriteZero extends Error {
        constructor(msg) {
          super(msg);
          this.name = "WriteZero";
        }
      };
      UnexpectedEof = class UnexpectedEof extends Error {
        constructor(msg) {
          super(msg);
          this.name = "UnexpectedEof";
        }
      };
      BadResource = class BadResource extends Error {
        constructor(msg) {
          super(msg);
          this.name = "BadResource";
        }
      };
      Http = class Http extends Error {
        constructor(msg) {
          super(msg);
          this.name = "Http";
        }
      };
      exports_7("errors", {
        NotFound: NotFound,
        PermissionDenied: PermissionDenied,
        ConnectionRefused: ConnectionRefused,
        ConnectionReset: ConnectionReset,
        ConnectionAborted: ConnectionAborted,
        NotConnected: NotConnected,
        AddrInUse: AddrInUse,
        AddrNotAvailable: AddrNotAvailable,
        BrokenPipe: BrokenPipe,
        AlreadyExists: AlreadyExists,
        InvalidData: InvalidData,
        TimedOut: TimedOut,
        Interrupted: Interrupted,
        WriteZero: WriteZero,
        UnexpectedEof: UnexpectedEof,
        BadResource: BadResource,
        Http: Http,
      });
    },
  };
});
System.register(
  "$deno$/ops/dispatch_minimal.ts",
  [
    "$deno$/util.ts",
    "$deno$/core.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/errors.ts",
  ],
  function (exports_8, context_8) {
    "use strict";
    let util,
      core_ts_2,
      text_encoding_ts_1,
      errors_ts_1,
      promiseTableMin,
      _nextPromiseId,
      decoder,
      scratch32,
      scratchBytes;
    const __moduleName = context_8 && context_8.id;
    function nextPromiseId() {
      return _nextPromiseId++;
    }
    function recordFromBufMinimal(ui8) {
      const header = ui8.subarray(0, 12);
      const buf32 = new Int32Array(
        header.buffer,
        header.byteOffset,
        header.byteLength / 4
      );
      const promiseId = buf32[0];
      const arg = buf32[1];
      const result = buf32[2];
      let err;
      if (arg < 0) {
        const kind = result;
        const message = decoder.decode(ui8.subarray(12));
        err = { kind, message };
      } else if (ui8.length != 12) {
        throw new errors_ts_1.errors.InvalidData("BadMessage");
      }
      return {
        promiseId,
        arg,
        result,
        err,
      };
    }
    exports_8("recordFromBufMinimal", recordFromBufMinimal);
    function unwrapResponse(res) {
      if (res.err != null) {
        throw new (errors_ts_1.getErrorClass(res.err.kind))(res.err.message);
      }
      return res.result;
    }
    function asyncMsgFromRust(ui8) {
      const record = recordFromBufMinimal(ui8);
      const { promiseId } = record;
      const promise = promiseTableMin[promiseId];
      delete promiseTableMin[promiseId];
      util.assert(promise);
      promise.resolve(record);
    }
    exports_8("asyncMsgFromRust", asyncMsgFromRust);
    async function sendAsyncMinimal(opId, arg, zeroCopy) {
      const promiseId = nextPromiseId(); // AKA cmdId
      scratch32[0] = promiseId;
      scratch32[1] = arg;
      scratch32[2] = 0; // result
      const promise = util.createResolvable();
      const buf = core_ts_2.core.dispatch(opId, scratchBytes, zeroCopy);
      if (buf) {
        const record = recordFromBufMinimal(buf);
        // Sync result.
        promise.resolve(record);
      } else {
        // Async result.
        promiseTableMin[promiseId] = promise;
      }
      const res = await promise;
      return unwrapResponse(res);
    }
    exports_8("sendAsyncMinimal", sendAsyncMinimal);
    function sendSyncMinimal(opId, arg, zeroCopy) {
      scratch32[0] = 0; // promiseId 0 indicates sync
      scratch32[1] = arg;
      const res = core_ts_2.core.dispatch(opId, scratchBytes, zeroCopy);
      const resRecord = recordFromBufMinimal(res);
      return unwrapResponse(resRecord);
    }
    exports_8("sendSyncMinimal", sendSyncMinimal);
    return {
      setters: [
        function (util_1) {
          util = util_1;
        },
        function (core_ts_2_1) {
          core_ts_2 = core_ts_2_1;
        },
        function (text_encoding_ts_1_1) {
          text_encoding_ts_1 = text_encoding_ts_1_1;
        },
        function (errors_ts_1_1) {
          errors_ts_1 = errors_ts_1_1;
        },
      ],
      execute: function () {
        // Using an object without a prototype because `Map` was causing GC problems.
        promiseTableMin = Object.create(null);
        // Note it's important that promiseId starts at 1 instead of 0, because sync
        // messages are indicated with promiseId 0. If we ever add wrap around logic for
        // overflows, this should be taken into account.
        _nextPromiseId = 1;
        decoder = new text_encoding_ts_1.TextDecoder();
        scratch32 = new Int32Array(3);
        scratchBytes = new Uint8Array(
          scratch32.buffer,
          scratch32.byteOffset,
          scratch32.byteLength
        );
        util.assert(scratchBytes.byteLength === scratch32.length * 4);
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/build.ts", [], function (exports_9, context_9) {
  "use strict";
  let build;
  const __moduleName = context_9 && context_9.id;
  function setBuildInfo(os, arch) {
    build.os = os;
    build.arch = arch;
    Object.freeze(build);
  }
  exports_9("setBuildInfo", setBuildInfo);
  return {
    setters: [],
    execute: function () {
      exports_9(
        "build",
        (build = {
          arch: "",
          os: "",
        })
      );
    },
  };
});
System.register("$deno$/version.ts", [], function (exports_10, context_10) {
  "use strict";
  let version;
  const __moduleName = context_10 && context_10.id;
  function setVersions(denoVersion, v8Version, tsVersion) {
    version.deno = denoVersion;
    version.v8 = v8Version;
    version.typescript = tsVersion;
    Object.freeze(version);
  }
  exports_10("setVersions", setVersions);
  return {
    setters: [],
    execute: function () {
      exports_10(
        "version",
        (version = {
          deno: "",
          v8: "",
          typescript: "",
        })
      );
    },
  };
});
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// Interfaces 100% copied from Go.
// Documentation liberally lifted from them too.
// Thank you! We love Go!
System.register("$deno$/io.ts", [], function (exports_11, context_11) {
  "use strict";
  let EOF, SeekMode;
  const __moduleName = context_11 && context_11.id;
  // https://golang.org/pkg/io/#Copy
  async function copy(dst, src) {
    let n = 0;
    const b = new Uint8Array(32 * 1024);
    let gotEOF = false;
    while (gotEOF === false) {
      const result = await src.read(b);
      if (result === EOF) {
        gotEOF = true;
      } else {
        n += await dst.write(b.subarray(0, result));
      }
    }
    return n;
  }
  exports_11("copy", copy);
  function toAsyncIterator(r) {
    const b = new Uint8Array(1024);
    return {
      [Symbol.asyncIterator]() {
        return this;
      },
      async next() {
        const result = await r.read(b);
        if (result === EOF) {
          return { value: new Uint8Array(), done: true };
        }
        return {
          value: b.subarray(0, result),
          done: false,
        };
      },
    };
  }
  exports_11("toAsyncIterator", toAsyncIterator);
  return {
    setters: [],
    execute: function () {
      exports_11("EOF", (EOF = Symbol("EOF")));
      // Seek whence values.
      // https://golang.org/pkg/io/#pkg-constants
      (function (SeekMode) {
        SeekMode[(SeekMode["SEEK_START"] = 0)] = "SEEK_START";
        SeekMode[(SeekMode["SEEK_CURRENT"] = 1)] = "SEEK_CURRENT";
        SeekMode[(SeekMode["SEEK_END"] = 2)] = "SEEK_END";
      })(SeekMode || (SeekMode = {}));
      exports_11("SeekMode", SeekMode);
    },
  };
});
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/buffer.ts",
  ["$deno$/io.ts", "$deno$/util.ts", "$deno$/web/text_encoding.ts"],
  function (exports_12, context_12) {
    "use strict";
    let io_ts_1, util_ts_1, text_encoding_ts_2, MIN_READ, MAX_SIZE, Buffer;
    const __moduleName = context_12 && context_12.id;
    // `off` is the offset into `dst` where it will at which to begin writing values
    // from `src`.
    // Returns the number of bytes copied.
    function copyBytes(dst, src, off = 0) {
      const r = dst.byteLength - off;
      if (src.byteLength > r) {
        src = src.subarray(0, r);
      }
      dst.set(src, off);
      return src.byteLength;
    }
    async function readAll(r) {
      const buf = new Buffer();
      await buf.readFrom(r);
      return buf.bytes();
    }
    exports_12("readAll", readAll);
    function readAllSync(r) {
      const buf = new Buffer();
      buf.readFromSync(r);
      return buf.bytes();
    }
    exports_12("readAllSync", readAllSync);
    async function writeAll(w, arr) {
      let nwritten = 0;
      while (nwritten < arr.length) {
        nwritten += await w.write(arr.subarray(nwritten));
      }
    }
    exports_12("writeAll", writeAll);
    function writeAllSync(w, arr) {
      let nwritten = 0;
      while (nwritten < arr.length) {
        nwritten += w.writeSync(arr.subarray(nwritten));
      }
    }
    exports_12("writeAllSync", writeAllSync);
    return {
      setters: [
        function (io_ts_1_1) {
          io_ts_1 = io_ts_1_1;
        },
        function (util_ts_1_1) {
          util_ts_1 = util_ts_1_1;
        },
        function (text_encoding_ts_2_1) {
          text_encoding_ts_2 = text_encoding_ts_2_1;
        },
      ],
      execute: function () {
        // MIN_READ is the minimum ArrayBuffer size passed to a read call by
        // buffer.ReadFrom. As long as the Buffer has at least MIN_READ bytes beyond
        // what is required to hold the contents of r, readFrom() will not grow the
        // underlying buffer.
        MIN_READ = 512;
        MAX_SIZE = 2 ** 32 - 2;
        Buffer = class Buffer {
          constructor(ab) {
            this.#off = 0; // read at buf[off], write at buf[buf.byteLength]
            this.#tryGrowByReslice = (n) => {
              const l = this.#buf.byteLength;
              if (n <= this.capacity - l) {
                this.#reslice(l + n);
                return l;
              }
              return -1;
            };
            this.#reslice = (len) => {
              util_ts_1.assert(len <= this.#buf.buffer.byteLength);
              this.#buf = new Uint8Array(this.#buf.buffer, 0, len);
            };
            this.#grow = (n) => {
              const m = this.length;
              // If buffer is empty, reset to recover space.
              if (m === 0 && this.#off !== 0) {
                this.reset();
              }
              // Fast: Try to grow by means of a reslice.
              const i = this.#tryGrowByReslice(n);
              if (i >= 0) {
                return i;
              }
              const c = this.capacity;
              if (n <= Math.floor(c / 2) - m) {
                // We can slide things down instead of allocating a new
                // ArrayBuffer. We only need m+n <= c to slide, but
                // we instead let capacity get twice as large so we
                // don't spend all our time copying.
                copyBytes(this.#buf, this.#buf.subarray(this.#off));
              } else if (c > MAX_SIZE - c - n) {
                throw new Error(
                  "The buffer cannot be grown beyond the maximum size."
                );
              } else {
                // Not enough space anywhere, we need to allocate.
                const buf = new Uint8Array(2 * c + n);
                copyBytes(buf, this.#buf.subarray(this.#off));
                this.#buf = buf;
              }
              // Restore this.#off and len(this.#buf).
              this.#off = 0;
              this.#reslice(m + n);
              return m;
            };
            if (ab == null) {
              this.#buf = new Uint8Array(0);
              return;
            }
            this.#buf = new Uint8Array(ab);
          }
          #buf; // contents are the bytes buf[off : len(buf)]
          #off; // read at buf[off], write at buf[buf.byteLength]
          bytes() {
            return this.#buf.subarray(this.#off);
          }
          toString() {
            const decoder = new text_encoding_ts_2.TextDecoder();
            return decoder.decode(this.#buf.subarray(this.#off));
          }
          empty() {
            return this.#buf.byteLength <= this.#off;
          }
          get length() {
            return this.#buf.byteLength - this.#off;
          }
          get capacity() {
            return this.#buf.buffer.byteLength;
          }
          truncate(n) {
            if (n === 0) {
              this.reset();
              return;
            }
            if (n < 0 || n > this.length) {
              throw Error("bytes.Buffer: truncation out of range");
            }
            this.#reslice(this.#off + n);
          }
          reset() {
            this.#reslice(0);
            this.#off = 0;
          }
          #tryGrowByReslice;
          #reslice;
          readSync(p) {
            if (this.empty()) {
              // Buffer is empty, reset to recover space.
              this.reset();
              if (p.byteLength === 0) {
                // this edge case is tested in 'bufferReadEmptyAtEOF' test
                return 0;
              }
              return io_ts_1.EOF;
            }
            const nread = copyBytes(p, this.#buf.subarray(this.#off));
            this.#off += nread;
            return nread;
          }
          read(p) {
            const rr = this.readSync(p);
            return Promise.resolve(rr);
          }
          writeSync(p) {
            const m = this.#grow(p.byteLength);
            return copyBytes(this.#buf, p, m);
          }
          write(p) {
            const n = this.writeSync(p);
            return Promise.resolve(n);
          }
          #grow;
          grow(n) {
            if (n < 0) {
              throw Error("Buffer.grow: negative count");
            }
            const m = this.#grow(n);
            this.#reslice(m);
          }
          async readFrom(r) {
            let n = 0;
            while (true) {
              try {
                const i = this.#grow(MIN_READ);
                this.#reslice(i);
                const fub = new Uint8Array(this.#buf.buffer, i);
                const nread = await r.read(fub);
                if (nread === io_ts_1.EOF) {
                  return n;
                }
                this.#reslice(i + nread);
                n += nread;
              } catch (e) {
                return n;
              }
            }
          }
          readFromSync(r) {
            let n = 0;
            while (true) {
              try {
                const i = this.#grow(MIN_READ);
                this.#reslice(i);
                const fub = new Uint8Array(this.#buf.buffer, i);
                const nread = r.readSync(fub);
                if (nread === io_ts_1.EOF) {
                  return n;
                }
                this.#reslice(i + nread);
                n += nread;
              } catch (e) {
                return n;
              }
            }
          }
        };
        exports_12("Buffer", Buffer);
      },
    };
  }
);
System.register(
  "$deno$/ops/fs/chmod.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_13, context_13) {
    "use strict";
    let dispatch_json_ts_1;
    const __moduleName = context_13 && context_13.id;
    function chmodSync(path, mode) {
      dispatch_json_ts_1.sendSync("op_chmod", { path, mode });
    }
    exports_13("chmodSync", chmodSync);
    async function chmod(path, mode) {
      await dispatch_json_ts_1.sendAsync("op_chmod", { path, mode });
    }
    exports_13("chmod", chmod);
    return {
      setters: [
        function (dispatch_json_ts_1_1) {
          dispatch_json_ts_1 = dispatch_json_ts_1_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/chown.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_14, context_14) {
    "use strict";
    let dispatch_json_ts_2;
    const __moduleName = context_14 && context_14.id;
    function chownSync(path, uid, gid) {
      dispatch_json_ts_2.sendSync("op_chown", { path, uid, gid });
    }
    exports_14("chownSync", chownSync);
    async function chown(path, uid, gid) {
      await dispatch_json_ts_2.sendAsync("op_chown", { path, uid, gid });
    }
    exports_14("chown", chown);
    return {
      setters: [
        function (dispatch_json_ts_2_1) {
          dispatch_json_ts_2 = dispatch_json_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/web/util.ts", [], function (exports_15, context_15) {
  "use strict";
  const __moduleName = context_15 && context_15.id;
  // @internal
  function isTypedArray(x) {
    return (
      x instanceof Int8Array ||
      x instanceof Uint8Array ||
      x instanceof Uint8ClampedArray ||
      x instanceof Int16Array ||
      x instanceof Uint16Array ||
      x instanceof Int32Array ||
      x instanceof Uint32Array ||
      x instanceof Float32Array ||
      x instanceof Float64Array
    );
  }
  exports_15("isTypedArray", isTypedArray);
  // @internal
  function requiredArguments(name, length, required) {
    if (length < required) {
      const errMsg = `${name} requires at least ${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present`;
      throw new TypeError(errMsg);
    }
  }
  exports_15("requiredArguments", requiredArguments);
  // @internal
  function immutableDefine(
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    o,
    p,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    value
  ) {
    Object.defineProperty(o, p, {
      value,
      configurable: false,
      writable: false,
    });
  }
  exports_15("immutableDefine", immutableDefine);
  // @internal
  function hasOwnProperty(obj, v) {
    if (obj == null) {
      return false;
    }
    return Object.prototype.hasOwnProperty.call(obj, v);
  }
  exports_15("hasOwnProperty", hasOwnProperty);
  /** Returns whether o is iterable.
   *
   * @internal */
  function isIterable(o) {
    // checks for null and undefined
    if (o == null) {
      return false;
    }
    return typeof o[Symbol.iterator] === "function";
  }
  exports_15("isIterable", isIterable);
  /** A helper function which ensures accessors are enumerable, as they normally
   * are not. */
  function defineEnumerableProps(Ctor, props) {
    for (const prop of props) {
      Reflect.defineProperty(Ctor.prototype, prop, { enumerable: true });
    }
  }
  exports_15("defineEnumerableProps", defineEnumerableProps);
  return {
    setters: [],
    execute: function () {},
  };
});
System.register(
  "$deno$/ops/resources.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_16, context_16) {
    "use strict";
    let dispatch_json_ts_3;
    const __moduleName = context_16 && context_16.id;
    function resources() {
      const res = dispatch_json_ts_3.sendSync("op_resources");
      const resources = {};
      for (const resourceTuple of res) {
        resources[resourceTuple[0]] = resourceTuple[1];
      }
      return resources;
    }
    exports_16("resources", resources);
    function close(rid) {
      dispatch_json_ts_3.sendSync("op_close", { rid });
    }
    exports_16("close", close);
    return {
      setters: [
        function (dispatch_json_ts_3_1) {
          dispatch_json_ts_3 = dispatch_json_ts_3_1;
        },
      ],
      execute: function () {},
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/io.ts",
  ["$deno$/ops/dispatch_minimal.ts", "$deno$/io.ts", "$deno$/runtime.ts"],
  function (exports_17, context_17) {
    "use strict";
    let dispatch_minimal_ts_1, io_ts_2, runtime_ts_1, OP_READ, OP_WRITE;
    const __moduleName = context_17 && context_17.id;
    function readSync(rid, buffer) {
      if (buffer.length == 0) {
        return 0;
      }
      if (OP_READ < 0) {
        OP_READ = runtime_ts_1.OPS_CACHE["op_read"];
      }
      const nread = dispatch_minimal_ts_1.sendSyncMinimal(OP_READ, rid, buffer);
      if (nread < 0) {
        throw new Error("read error");
      } else if (nread == 0) {
        return io_ts_2.EOF;
      } else {
        return nread;
      }
    }
    exports_17("readSync", readSync);
    async function read(rid, buffer) {
      if (buffer.length == 0) {
        return 0;
      }
      if (OP_READ < 0) {
        OP_READ = runtime_ts_1.OPS_CACHE["op_read"];
      }
      const nread = await dispatch_minimal_ts_1.sendAsyncMinimal(
        OP_READ,
        rid,
        buffer
      );
      if (nread < 0) {
        throw new Error("read error");
      } else if (nread == 0) {
        return io_ts_2.EOF;
      } else {
        return nread;
      }
    }
    exports_17("read", read);
    function writeSync(rid, data) {
      if (OP_WRITE < 0) {
        OP_WRITE = runtime_ts_1.OPS_CACHE["op_write"];
      }
      const result = dispatch_minimal_ts_1.sendSyncMinimal(OP_WRITE, rid, data);
      if (result < 0) {
        throw new Error("write error");
      } else {
        return result;
      }
    }
    exports_17("writeSync", writeSync);
    async function write(rid, data) {
      if (OP_WRITE < 0) {
        OP_WRITE = runtime_ts_1.OPS_CACHE["op_write"];
      }
      const result = await dispatch_minimal_ts_1.sendAsyncMinimal(
        OP_WRITE,
        rid,
        data
      );
      if (result < 0) {
        throw new Error("write error");
      } else {
        return result;
      }
    }
    exports_17("write", write);
    return {
      setters: [
        function (dispatch_minimal_ts_1_1) {
          dispatch_minimal_ts_1 = dispatch_minimal_ts_1_1;
        },
        function (io_ts_2_1) {
          io_ts_2 = io_ts_2_1;
        },
        function (runtime_ts_1_1) {
          runtime_ts_1 = runtime_ts_1_1;
        },
      ],
      execute: function () {
        // This is done because read/write are extremely performance sensitive.
        OP_READ = -1;
        OP_WRITE = -1;
      },
    };
  }
);
System.register(
  "$deno$/ops/fs/seek.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_18, context_18) {
    "use strict";
    let dispatch_json_ts_4;
    const __moduleName = context_18 && context_18.id;
    function seekSync(rid, offset, whence) {
      return dispatch_json_ts_4.sendSync("op_seek", { rid, offset, whence });
    }
    exports_18("seekSync", seekSync);
    function seek(rid, offset, whence) {
      return dispatch_json_ts_4.sendAsync("op_seek", { rid, offset, whence });
    }
    exports_18("seek", seek);
    return {
      setters: [
        function (dispatch_json_ts_4_1) {
          dispatch_json_ts_4 = dispatch_json_ts_4_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/open.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_19, context_19) {
    "use strict";
    let dispatch_json_ts_5;
    const __moduleName = context_19 && context_19.id;
    function openSync(path, openMode, options) {
      const mode = options?.mode;
      return dispatch_json_ts_5.sendSync("op_open", {
        path,
        options,
        openMode,
        mode,
      });
    }
    exports_19("openSync", openSync);
    function open(path, openMode, options) {
      const mode = options?.mode;
      return dispatch_json_ts_5.sendAsync("op_open", {
        path,
        options,
        openMode,
        mode,
      });
    }
    exports_19("open", open);
    return {
      setters: [
        function (dispatch_json_ts_5_1) {
          dispatch_json_ts_5 = dispatch_json_ts_5_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/files.ts",
  [
    "$deno$/ops/resources.ts",
    "$deno$/ops/io.ts",
    "$deno$/ops/fs/seek.ts",
    "$deno$/ops/fs/open.ts",
  ],
  function (exports_20, context_20) {
    "use strict";
    let resources_ts_1, io_ts_3, seek_ts_1, open_ts_1, File;
    const __moduleName = context_20 && context_20.id;
    /**@internal*/
    function openSync(path, modeOrOptions = "r") {
      let openMode = undefined;
      let options = undefined;
      if (typeof modeOrOptions === "string") {
        openMode = modeOrOptions;
      } else {
        checkOpenOptions(modeOrOptions);
        options = modeOrOptions;
      }
      const rid = open_ts_1.openSync(path, openMode, options);
      return new File(rid);
    }
    exports_20("openSync", openSync);
    /**@internal*/
    async function open(path, modeOrOptions = "r") {
      let openMode = undefined;
      let options = undefined;
      if (typeof modeOrOptions === "string") {
        openMode = modeOrOptions;
      } else {
        checkOpenOptions(modeOrOptions);
        options = modeOrOptions;
      }
      const rid = await open_ts_1.open(path, openMode, options);
      return new File(rid);
    }
    exports_20("open", open);
    function createSync(path) {
      return openSync(path, "w+");
    }
    exports_20("createSync", createSync);
    function create(path) {
      return open(path, "w+");
    }
    exports_20("create", create);
    function checkOpenOptions(options) {
      if (Object.values(options).filter((val) => val === true).length === 0) {
        throw new Error("OpenOptions requires at least one option to be true");
      }
      if (options.truncate && !options.write) {
        throw new Error("'truncate' option requires 'write' option");
      }
      const createOrCreateNewWithoutWriteOrAppend =
        (options.create || options.createNew) &&
        !(options.write || options.append);
      if (createOrCreateNewWithoutWriteOrAppend) {
        throw new Error(
          "'create' or 'createNew' options require 'write' or 'append' option"
        );
      }
    }
    return {
      setters: [
        function (resources_ts_1_1) {
          resources_ts_1 = resources_ts_1_1;
        },
        function (io_ts_3_1) {
          io_ts_3 = io_ts_3_1;
        },
        function (seek_ts_1_1) {
          seek_ts_1 = seek_ts_1_1;
          exports_20({
            seek: seek_ts_1_1["seek"],
            seekSync: seek_ts_1_1["seekSync"],
          });
        },
        function (open_ts_1_1) {
          open_ts_1 = open_ts_1_1;
        },
      ],
      execute: function () {
        File = class File {
          constructor(rid) {
            this.rid = rid;
          }
          write(p) {
            return io_ts_3.write(this.rid, p);
          }
          writeSync(p) {
            return io_ts_3.writeSync(this.rid, p);
          }
          read(p) {
            return io_ts_3.read(this.rid, p);
          }
          readSync(p) {
            return io_ts_3.readSync(this.rid, p);
          }
          seek(offset, whence) {
            return seek_ts_1.seek(this.rid, offset, whence);
          }
          seekSync(offset, whence) {
            return seek_ts_1.seekSync(this.rid, offset, whence);
          }
          close() {
            resources_ts_1.close(this.rid);
          }
        };
        exports_20("File", File);
        exports_20("stdin", new File(0));
        exports_20("stdout", new File(1));
        exports_20("stderr", new File(2));
      },
    };
  }
);
// Copyright Joyent, Inc. and other Node contributors. MIT license.
// Forked from Node's lib/internal/cli_table.js
System.register(
  "$deno$/web/console_table.ts",
  ["$deno$/web/text_encoding.ts", "$deno$/web/util.ts"],
  function (exports_21, context_21) {
    "use strict";
    let text_encoding_ts_3, util_ts_2, encoder, tableChars, colorRegExp;
    const __moduleName = context_21 && context_21.id;
    function removeColors(str) {
      return str.replace(colorRegExp, "");
    }
    function countBytes(str) {
      const normalized = removeColors(String(str)).normalize("NFC");
      return encoder.encode(normalized).byteLength;
    }
    function renderRow(row, columnWidths) {
      let out = tableChars.left;
      for (let i = 0; i < row.length; i++) {
        const cell = row[i];
        const len = countBytes(cell);
        const needed = (columnWidths[i] - len) / 2;
        // round(needed) + ceil(needed) will always add up to the amount
        // of spaces we need while also left justifying the output.
        out += `${" ".repeat(needed)}${cell}${" ".repeat(Math.ceil(needed))}`;
        if (i !== row.length - 1) {
          out += tableChars.middle;
        }
      }
      out += tableChars.right;
      return out;
    }
    function cliTable(head, columns) {
      const rows = [];
      const columnWidths = head.map((h) => countBytes(h));
      const longestColumn = columns.reduce((n, a) => Math.max(n, a.length), 0);
      for (let i = 0; i < head.length; i++) {
        const column = columns[i];
        for (let j = 0; j < longestColumn; j++) {
          if (rows[j] === undefined) {
            rows[j] = [];
          }
          const value = (rows[j][i] = util_ts_2.hasOwnProperty(column, j)
            ? column[j]
            : "");
          const width = columnWidths[i] || 0;
          const counted = countBytes(value);
          columnWidths[i] = Math.max(width, counted);
        }
      }
      const divider = columnWidths.map((i) =>
        tableChars.middleMiddle.repeat(i + 2)
      );
      let result =
        `${tableChars.topLeft}${divider.join(tableChars.topMiddle)}` +
        `${tableChars.topRight}\n${renderRow(head, columnWidths)}\n` +
        `${tableChars.leftMiddle}${divider.join(tableChars.rowMiddle)}` +
        `${tableChars.rightMiddle}\n`;
      for (const row of rows) {
        result += `${renderRow(row, columnWidths)}\n`;
      }
      result +=
        `${tableChars.bottomLeft}${divider.join(tableChars.bottomMiddle)}` +
        tableChars.bottomRight;
      return result;
    }
    exports_21("cliTable", cliTable);
    return {
      setters: [
        function (text_encoding_ts_3_1) {
          text_encoding_ts_3 = text_encoding_ts_3_1;
        },
        function (util_ts_2_1) {
          util_ts_2 = util_ts_2_1;
        },
      ],
      execute: function () {
        encoder = new text_encoding_ts_3.TextEncoder();
        tableChars = {
          middleMiddle: "─",
          rowMiddle: "┼",
          topRight: "┐",
          topLeft: "┌",
          leftMiddle: "├",
          topMiddle: "┬",
          bottomRight: "┘",
          bottomLeft: "└",
          bottomMiddle: "┴",
          rightMiddle: "┤",
          left: "│ ",
          right: " │",
          middle: " │ ",
        };
        colorRegExp = /\u001b\[\d\d?m/g;
      },
    };
  }
);
System.register("$deno$/internals.ts", [], function (exports_22, context_22) {
  "use strict";
  let internalObject;
  const __moduleName = context_22 && context_22.id;
  // Register a field to internalObject for test access,
  // through Deno[Deno.symbols.internal][name].
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function exposeForTest(name, value) {
    Object.defineProperty(internalObject, name, {
      value,
      enumerable: false,
    });
  }
  exports_22("exposeForTest", exposeForTest);
  return {
    setters: [],
    execute: function () {
      // Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
      exports_22("internalSymbol", Symbol("Deno.internal"));
      // The object where all the internal fields for testing will be living.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      exports_22("internalObject", (internalObject = {}));
    },
  };
});
System.register("$deno$/web/promise.ts", [], function (exports_23, context_23) {
  "use strict";
  let PromiseState;
  const __moduleName = context_23 && context_23.id;
  return {
    setters: [],
    execute: function () {
      (function (PromiseState) {
        PromiseState[(PromiseState["Pending"] = 0)] = "Pending";
        PromiseState[(PromiseState["Fulfilled"] = 1)] = "Fulfilled";
        PromiseState[(PromiseState["Rejected"] = 2)] = "Rejected";
      })(PromiseState || (PromiseState = {}));
      exports_23("PromiseState", PromiseState);
    },
  };
});
System.register(
  "$deno$/web/console.ts",
  [
    "$deno$/web/util.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/files.ts",
    "$deno$/web/console_table.ts",
    "$deno$/internals.ts",
    "$deno$/web/promise.ts",
  ],
  function (exports_24, context_24) {
    "use strict";
    let _a,
      util_ts_3,
      text_encoding_ts_4,
      files_ts_1,
      console_table_ts_1,
      internals_ts_1,
      promise_ts_1,
      DEFAULT_MAX_DEPTH,
      LINE_BREAKING_LENGTH,
      MAX_ITERABLE_LENGTH,
      MIN_GROUP_LENGTH,
      STR_ABBREVIATE_SIZE,
      CHAR_PERCENT,
      CHAR_LOWERCASE_S,
      CHAR_LOWERCASE_D,
      CHAR_LOWERCASE_I,
      CHAR_LOWERCASE_F,
      CHAR_LOWERCASE_O,
      CHAR_UPPERCASE_O,
      CHAR_LOWERCASE_C,
      PROMISE_STRING_BASE_LENGTH,
      CSI,
      countMap,
      timerMap,
      isConsoleInstance,
      Console,
      customInspect;
    const __moduleName = context_24 && context_24.id;
    /* eslint-disable @typescript-eslint/no-use-before-define */
    function cursorTo(stream, _x, _y) {
      const uint8 = new text_encoding_ts_4.TextEncoder().encode(CSI.kClear);
      stream.writeSync(uint8);
    }
    function clearScreenDown(stream) {
      const uint8 = new text_encoding_ts_4.TextEncoder().encode(
        CSI.kClearScreenDown
      );
      stream.writeSync(uint8);
    }
    function getClassInstanceName(instance) {
      if (typeof instance !== "object") {
        return "";
      }
      if (!instance) {
        return "";
      }
      const proto = Object.getPrototypeOf(instance);
      if (proto && proto.constructor) {
        return proto.constructor.name; // could be "Object" or "Array"
      }
      return "";
    }
    function createFunctionString(value, _ctx) {
      // Might be Function/AsyncFunction/GeneratorFunction
      const cstrName = Object.getPrototypeOf(value).constructor.name;
      if (value.name && value.name !== "anonymous") {
        // from MDN spec
        return `[${cstrName}: ${value.name}]`;
      }
      return `[${cstrName}]`;
    }
    function createIterableString(value, ctx, level, maxLevel, config) {
      if (level >= maxLevel) {
        return `[${config.typeName}]`;
      }
      ctx.add(value);
      const entries = [];
      const iter = value.entries();
      let entriesLength = 0;
      const next = () => {
        return iter.next();
      };
      for (const el of iter) {
        if (entriesLength < MAX_ITERABLE_LENGTH) {
          entries.push(
            config.entryHandler(el, ctx, level + 1, maxLevel, next.bind(iter))
          );
        }
        entriesLength++;
      }
      ctx.delete(value);
      if (entriesLength > MAX_ITERABLE_LENGTH) {
        const nmore = entriesLength - MAX_ITERABLE_LENGTH;
        entries.push(`... ${nmore} more items`);
      }
      const iPrefix = `${config.displayName ? config.displayName + " " : ""}`;
      let iContent;
      if (config.group && entries.length > MIN_GROUP_LENGTH) {
        const groups = groupEntries(entries, level, value);
        const initIndentation = `\n${"  ".repeat(level + 1)}`;
        const entryIndetation = `,\n${"  ".repeat(level + 1)}`;
        const closingIndentation = `\n${"  ".repeat(level)}`;
        iContent = `${initIndentation}${groups.join(
          entryIndetation
        )}${closingIndentation}`;
      } else {
        iContent = entries.length === 0 ? "" : ` ${entries.join(", ")} `;
        if (iContent.length > LINE_BREAKING_LENGTH) {
          const initIndentation = `\n${" ".repeat(level + 1)}`;
          const entryIndetation = `,\n${" ".repeat(level + 1)}`;
          const closingIndentation = `\n`;
          iContent = `${initIndentation}${entries.join(
            entryIndetation
          )}${closingIndentation}`;
        }
      }
      return `${iPrefix}${config.delims[0]}${iContent}${config.delims[1]}`;
    }
    // Ported from Node.js
    // Copyright Node.js contributors. All rights reserved.
    function groupEntries(entries, level, value) {
      let totalLength = 0;
      let maxLength = 0;
      let entriesLength = entries.length;
      if (MAX_ITERABLE_LENGTH < entriesLength) {
        // This makes sure the "... n more items" part is not taken into account.
        entriesLength--;
      }
      const separatorSpace = 2; // Add 1 for the space and 1 for the separator.
      const dataLen = new Array(entriesLength);
      // Calculate the total length of all output entries and the individual max
      // entries length of all output entries. In future colors should be taken
      // here into the account
      for (let i = 0; i < entriesLength; i++) {
        const len = entries[i].length;
        dataLen[i] = len;
        totalLength += len + separatorSpace;
        if (maxLength < len) maxLength = len;
      }
      // Add two to `maxLength` as we add a single whitespace character plus a comma
      // in-between two entries.
      const actualMax = maxLength + separatorSpace;
      // Check if at least three entries fit next to each other and prevent grouping
      // of arrays that contains entries of very different length (i.e., if a single
      // entry is longer than 1/5 of all other entries combined). Otherwise the
      // space in-between small entries would be enormous.
      if (
        actualMax * 3 + (level + 1) < LINE_BREAKING_LENGTH &&
        (totalLength / actualMax > 5 || maxLength <= 6)
      ) {
        const approxCharHeights = 2.5;
        const averageBias = Math.sqrt(actualMax - totalLength / entries.length);
        const biasedMax = Math.max(actualMax - 3 - averageBias, 1);
        // Dynamically check how many columns seem possible.
        const columns = Math.min(
          // Ideally a square should be drawn. We expect a character to be about 2.5
          // times as high as wide. This is the area formula to calculate a square
          // which contains n rectangles of size `actualMax * approxCharHeights`.
          // Divide that by `actualMax` to receive the correct number of columns.
          // The added bias increases the columns for short entries.
          Math.round(
            Math.sqrt(approxCharHeights * biasedMax * entriesLength) / biasedMax
          ),
          // Do not exceed the breakLength.
          Math.floor((LINE_BREAKING_LENGTH - (level + 1)) / actualMax),
          // Limit the columns to a maximum of fifteen.
          15
        );
        // Return with the original output if no grouping should happen.
        if (columns <= 1) {
          return entries;
        }
        const tmp = [];
        const maxLineLength = [];
        for (let i = 0; i < columns; i++) {
          let lineMaxLength = 0;
          for (let j = i; j < entries.length; j += columns) {
            if (dataLen[j] > lineMaxLength) lineMaxLength = dataLen[j];
          }
          lineMaxLength += separatorSpace;
          maxLineLength[i] = lineMaxLength;
        }
        let order = "padStart";
        if (value !== undefined) {
          for (let i = 0; i < entries.length; i++) {
            //@ts-ignore
            if (typeof value[i] !== "number" && typeof value[i] !== "bigint") {
              order = "padEnd";
              break;
            }
          }
        }
        // Each iteration creates a single line of grouped entries.
        for (let i = 0; i < entriesLength; i += columns) {
          // The last lines may contain less entries than columns.
          const max = Math.min(i + columns, entriesLength);
          let str = "";
          let j = i;
          for (; j < max - 1; j++) {
            // In future, colors should be taken here into the account
            const padding = maxLineLength[j - i];
            //@ts-ignore
            str += `${entries[j]}, `[order](padding, " ");
          }
          if (order === "padStart") {
            const padding =
              maxLineLength[j - i] +
              entries[j].length -
              dataLen[j] -
              separatorSpace;
            str += entries[j].padStart(padding, " ");
          } else {
            str += entries[j];
          }
          tmp.push(str);
        }
        if (MAX_ITERABLE_LENGTH < entries.length) {
          tmp.push(entries[entriesLength]);
        }
        entries = tmp;
      }
      return entries;
    }
    function stringify(value, ctx, level, maxLevel) {
      switch (typeof value) {
        case "string":
          return value;
        case "number":
          // Special handling of -0
          return Object.is(value, -0) ? "-0" : `${value}`;
        case "boolean":
        case "undefined":
        case "symbol":
          return String(value);
        case "bigint":
          return `${value}n`;
        case "function":
          return createFunctionString(value, ctx);
        case "object":
          if (value === null) {
            return "null";
          }
          if (ctx.has(value)) {
            return "[Circular]";
          }
          return createObjectString(value, ctx, level, maxLevel);
        default:
          return "[Not Implemented]";
      }
    }
    // Print strings when they are inside of arrays or objects with quotes
    function stringifyWithQuotes(value, ctx, level, maxLevel) {
      switch (typeof value) {
        case "string":
          const trunc =
            value.length > STR_ABBREVIATE_SIZE
              ? value.slice(0, STR_ABBREVIATE_SIZE) + "..."
              : value;
          return JSON.stringify(trunc);
        default:
          return stringify(value, ctx, level, maxLevel);
      }
    }
    function createArrayString(value, ctx, level, maxLevel) {
      const printConfig = {
        typeName: "Array",
        displayName: "",
        delims: ["[", "]"],
        entryHandler: (entry, ctx, level, maxLevel, next) => {
          const [index, val] = entry;
          let i = index;
          if (!value.hasOwnProperty(i)) {
            i++;
            while (!value.hasOwnProperty(i) && i < value.length) {
              next();
              i++;
            }
            const emptyItems = i - index;
            const ending = emptyItems > 1 ? "s" : "";
            return `<${emptyItems} empty item${ending}>`;
          } else {
            return stringifyWithQuotes(val, ctx, level + 1, maxLevel);
          }
        },
        group: true,
      };
      return createIterableString(value, ctx, level, maxLevel, printConfig);
    }
    function createTypedArrayString(
      typedArrayName,
      value,
      ctx,
      level,
      maxLevel
    ) {
      const valueLength = value.length;
      const printConfig = {
        typeName: typedArrayName,
        displayName: `${typedArrayName}(${valueLength})`,
        delims: ["[", "]"],
        entryHandler: (entry, ctx, level, maxLevel) => {
          const [_, val] = entry;
          return stringifyWithQuotes(val, ctx, level + 1, maxLevel);
        },
        group: true,
      };
      return createIterableString(value, ctx, level, maxLevel, printConfig);
    }
    function createSetString(value, ctx, level, maxLevel) {
      const printConfig = {
        typeName: "Set",
        displayName: "Set",
        delims: ["{", "}"],
        entryHandler: (entry, ctx, level, maxLevel) => {
          const [_, val] = entry;
          return stringifyWithQuotes(val, ctx, level + 1, maxLevel);
        },
        group: false,
      };
      return createIterableString(value, ctx, level, maxLevel, printConfig);
    }
    function createMapString(value, ctx, level, maxLevel) {
      const printConfig = {
        typeName: "Map",
        displayName: "Map",
        delims: ["{", "}"],
        entryHandler: (entry, ctx, level, maxLevel) => {
          const [key, val] = entry;
          return `${stringifyWithQuotes(
            key,
            ctx,
            level + 1,
            maxLevel
          )} => ${stringifyWithQuotes(val, ctx, level + 1, maxLevel)}`;
        },
        group: false,
      };
      //@ts-ignore
      return createIterableString(value, ctx, level, maxLevel, printConfig);
    }
    function createWeakSetString() {
      return "WeakSet { [items unknown] }"; // as seen in Node
    }
    function createWeakMapString() {
      return "WeakMap { [items unknown] }"; // as seen in Node
    }
    function createDateString(value) {
      // without quotes, ISO format
      return value.toISOString();
    }
    function createRegExpString(value) {
      return value.toString();
    }
    /* eslint-disable @typescript-eslint/ban-types */
    function createStringWrapperString(value) {
      return `[String: "${value.toString()}"]`;
    }
    function createBooleanWrapperString(value) {
      return `[Boolean: ${value.toString()}]`;
    }
    function createNumberWrapperString(value) {
      return `[Number: ${value.toString()}]`;
    }
    /* eslint-enable @typescript-eslint/ban-types */
    function createPromiseString(value, ctx, level, maxLevel) {
      const [state, result] = Deno.core.getPromiseDetails(value);
      if (state === promise_ts_1.PromiseState.Pending) {
        return "Promise { <pending> }";
      }
      const prefix =
        state === promise_ts_1.PromiseState.Fulfilled ? "" : "<rejected> ";
      const str = `${prefix}${stringifyWithQuotes(
        result,
        ctx,
        level + 1,
        maxLevel
      )}`;
      if (str.length + PROMISE_STRING_BASE_LENGTH > LINE_BREAKING_LENGTH) {
        return `Promise {\n${" ".repeat(level + 1)}${str}\n}`;
      }
      return `Promise { ${str} }`;
    }
    // TODO: Proxy
    function createRawObjectString(value, ctx, level, maxLevel) {
      if (level >= maxLevel) {
        return "[Object]";
      }
      ctx.add(value);
      let baseString = "";
      let shouldShowDisplayName = false;
      // @ts-ignore
      let displayName = value[Symbol.toStringTag];
      if (!displayName) {
        displayName = getClassInstanceName(value);
      }
      if (
        displayName &&
        displayName !== "Object" &&
        displayName !== "anonymous"
      ) {
        shouldShowDisplayName = true;
      }
      const entries = [];
      const stringKeys = Object.keys(value);
      const symbolKeys = Object.getOwnPropertySymbols(value);
      for (const key of stringKeys) {
        entries.push(
          `${key}: ${stringifyWithQuotes(value[key], ctx, level + 1, maxLevel)}`
        );
      }
      for (const key of symbolKeys) {
        entries.push(
          `${key.toString()}: ${stringifyWithQuotes(
            // @ts-ignore
            value[key],
            ctx,
            level + 1,
            maxLevel
          )}`
        );
      }
      const totalLength = entries.length + level + entries.join("").length;
      ctx.delete(value);
      if (entries.length === 0) {
        baseString = "{}";
      } else if (totalLength > LINE_BREAKING_LENGTH) {
        const entryIndent = " ".repeat(level + 1);
        const closingIndent = " ".repeat(level);
        baseString = `{\n${entryIndent}${entries.join(
          `,\n${entryIndent}`
        )}\n${closingIndent}}`;
      } else {
        baseString = `{ ${entries.join(", ")} }`;
      }
      if (shouldShowDisplayName) {
        baseString = `${displayName} ${baseString}`;
      }
      return baseString;
    }
    function createObjectString(value, ...args) {
      if (
        customInspect in value &&
        typeof value[customInspect] === "function"
      ) {
        try {
          return String(value[customInspect]());
        } catch {}
      }
      if (value instanceof Error) {
        return String(value.stack);
      } else if (Array.isArray(value)) {
        return createArrayString(value, ...args);
      } else if (value instanceof Number) {
        return createNumberWrapperString(value);
      } else if (value instanceof Boolean) {
        return createBooleanWrapperString(value);
      } else if (value instanceof String) {
        return createStringWrapperString(value);
      } else if (value instanceof Promise) {
        return createPromiseString(value, ...args);
      } else if (value instanceof RegExp) {
        return createRegExpString(value);
      } else if (value instanceof Date) {
        return createDateString(value);
      } else if (value instanceof Set) {
        return createSetString(value, ...args);
      } else if (value instanceof Map) {
        return createMapString(value, ...args);
      } else if (value instanceof WeakSet) {
        return createWeakSetString();
      } else if (value instanceof WeakMap) {
        return createWeakMapString();
      } else if (util_ts_3.isTypedArray(value)) {
        return createTypedArrayString(
          Object.getPrototypeOf(value).constructor.name,
          value,
          ...args
        );
      } else {
        // Otherwise, default object formatting
        return createRawObjectString(value, ...args);
      }
    }
    function stringifyArgs(
      args,
      { depth = DEFAULT_MAX_DEPTH, indentLevel = 0 } = {}
    ) {
      const first = args[0];
      let a = 0;
      let str = "";
      let join = "";
      if (typeof first === "string") {
        let tempStr;
        let lastPos = 0;
        for (let i = 0; i < first.length - 1; i++) {
          if (first.charCodeAt(i) === CHAR_PERCENT) {
            const nextChar = first.charCodeAt(++i);
            if (a + 1 !== args.length) {
              switch (nextChar) {
                case CHAR_LOWERCASE_S:
                  // format as a string
                  tempStr = String(args[++a]);
                  break;
                case CHAR_LOWERCASE_D:
                case CHAR_LOWERCASE_I:
                  // format as an integer
                  const tempInteger = args[++a];
                  if (typeof tempInteger === "bigint") {
                    tempStr = `${tempInteger}n`;
                  } else if (typeof tempInteger === "symbol") {
                    tempStr = "NaN";
                  } else {
                    tempStr = `${parseInt(String(tempInteger), 10)}`;
                  }
                  break;
                case CHAR_LOWERCASE_F:
                  // format as a floating point value
                  const tempFloat = args[++a];
                  if (typeof tempFloat === "symbol") {
                    tempStr = "NaN";
                  } else {
                    tempStr = `${parseFloat(String(tempFloat))}`;
                  }
                  break;
                case CHAR_LOWERCASE_O:
                case CHAR_UPPERCASE_O:
                  // format as an object
                  tempStr = stringify(args[++a], new Set(), 0, depth);
                  break;
                case CHAR_PERCENT:
                  str += first.slice(lastPos, i);
                  lastPos = i + 1;
                  continue;
                case CHAR_LOWERCASE_C:
                  // TODO: applies CSS style rules to the output string as specified
                  continue;
                default:
                  // any other character is not a correct placeholder
                  continue;
              }
              if (lastPos !== i - 1) {
                str += first.slice(lastPos, i - 1);
              }
              str += tempStr;
              lastPos = i + 1;
            } else if (nextChar === CHAR_PERCENT) {
              str += first.slice(lastPos, i);
              lastPos = i + 1;
            }
          }
        }
        if (lastPos !== 0) {
          a++;
          join = " ";
          if (lastPos < first.length) {
            str += first.slice(lastPos);
          }
        }
      }
      while (a < args.length) {
        const value = args[a];
        str += join;
        if (typeof value === "string") {
          str += value;
        } else {
          // use default maximum depth for null or undefined argument
          str += stringify(value, new Set(), 0, depth);
        }
        join = " ";
        a++;
      }
      if (indentLevel > 0) {
        const groupIndent = " ".repeat(indentLevel);
        if (str.indexOf("\n") !== -1) {
          str = str.replace(/\n/g, `\n${groupIndent}`);
        }
        str = groupIndent + str;
      }
      return str;
    }
    exports_24("stringifyArgs", stringifyArgs);
    function inspect(value, { depth = DEFAULT_MAX_DEPTH } = {}) {
      if (typeof value === "string") {
        return value;
      } else {
        return stringify(value, new Set(), 0, depth);
      }
    }
    exports_24("inspect", inspect);
    return {
      setters: [
        function (util_ts_3_1) {
          util_ts_3 = util_ts_3_1;
        },
        function (text_encoding_ts_4_1) {
          text_encoding_ts_4 = text_encoding_ts_4_1;
        },
        function (files_ts_1_1) {
          files_ts_1 = files_ts_1_1;
        },
        function (console_table_ts_1_1) {
          console_table_ts_1 = console_table_ts_1_1;
        },
        function (internals_ts_1_1) {
          internals_ts_1 = internals_ts_1_1;
        },
        function (promise_ts_1_1) {
          promise_ts_1 = promise_ts_1_1;
        },
      ],
      execute: function () {
        DEFAULT_MAX_DEPTH = 4; // Default depth of logging nested objects
        LINE_BREAKING_LENGTH = 80;
        MAX_ITERABLE_LENGTH = 100;
        MIN_GROUP_LENGTH = 6;
        STR_ABBREVIATE_SIZE = 100;
        // Char codes
        CHAR_PERCENT = 37; /* % */
        CHAR_LOWERCASE_S = 115; /* s */
        CHAR_LOWERCASE_D = 100; /* d */
        CHAR_LOWERCASE_I = 105; /* i */
        CHAR_LOWERCASE_F = 102; /* f */
        CHAR_LOWERCASE_O = 111; /* o */
        CHAR_UPPERCASE_O = 79; /* O */
        CHAR_LOWERCASE_C = 99; /* c */
        PROMISE_STRING_BASE_LENGTH = 12;
        CSI = class CSI {};
        exports_24("CSI", CSI);
        CSI.kClear = "\x1b[1;1H";
        CSI.kClearScreenDown = "\x1b[0J";
        countMap = new Map();
        timerMap = new Map();
        isConsoleInstance = Symbol("isConsoleInstance");
        Console = class Console {
          constructor(printFunc) {
            this[_a] = false;
            this.log = (...args) => {
              this.#printFunc(
                stringifyArgs(args, {
                  indentLevel: this.indentLevel,
                }) + "\n",
                false
              );
            };
            this.debug = this.log;
            this.info = this.log;
            this.dir = (obj, options = {}) => {
              this.#printFunc(stringifyArgs([obj], options) + "\n", false);
            };
            this.dirxml = this.dir;
            this.warn = (...args) => {
              this.#printFunc(
                stringifyArgs(args, {
                  indentLevel: this.indentLevel,
                }) + "\n",
                true
              );
            };
            this.error = this.warn;
            this.assert = (condition = false, ...args) => {
              if (condition) {
                return;
              }
              if (args.length === 0) {
                this.error("Assertion failed");
                return;
              }
              const [first, ...rest] = args;
              if (typeof first === "string") {
                this.error(`Assertion failed: ${first}`, ...rest);
                return;
              }
              this.error(`Assertion failed:`, ...args);
            };
            this.count = (label = "default") => {
              label = String(label);
              if (countMap.has(label)) {
                const current = countMap.get(label) || 0;
                countMap.set(label, current + 1);
              } else {
                countMap.set(label, 1);
              }
              this.info(`${label}: ${countMap.get(label)}`);
            };
            this.countReset = (label = "default") => {
              label = String(label);
              if (countMap.has(label)) {
                countMap.set(label, 0);
              } else {
                this.warn(`Count for '${label}' does not exist`);
              }
            };
            this.table = (data, properties) => {
              if (properties !== undefined && !Array.isArray(properties)) {
                throw new Error(
                  "The 'properties' argument must be of type Array. " +
                    "Received type string"
                );
              }
              if (data === null || typeof data !== "object") {
                return this.log(data);
              }
              const objectValues = {};
              const indexKeys = [];
              const values = [];
              const stringifyValue = (value) =>
                stringifyWithQuotes(value, new Set(), 0, 1);
              const toTable = (header, body) =>
                this.log(console_table_ts_1.cliTable(header, body));
              const createColumn = (value, shift) => [
                ...(shift ? [...new Array(shift)].map(() => "") : []),
                stringifyValue(value),
              ];
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              let resultData;
              const isSet = data instanceof Set;
              const isMap = data instanceof Map;
              const valuesKey = "Values";
              const indexKey = isSet || isMap ? "(iteration index)" : "(index)";
              if (data instanceof Set) {
                resultData = [...data];
              } else if (data instanceof Map) {
                let idx = 0;
                resultData = {};
                data.forEach((v, k) => {
                  resultData[idx] = { Key: k, Values: v };
                  idx++;
                });
              } else {
                resultData = data;
              }
              Object.keys(resultData).forEach((k, idx) => {
                const value = resultData[k];
                if (value !== null && typeof value === "object") {
                  Object.entries(value).forEach(([k, v]) => {
                    if (properties && !properties.includes(k)) {
                      return;
                    }
                    if (objectValues[k]) {
                      objectValues[k].push(stringifyValue(v));
                    } else {
                      objectValues[k] = createColumn(v, idx);
                    }
                  });
                  values.push("");
                } else {
                  values.push(stringifyValue(value));
                }
                indexKeys.push(k);
              });
              const headerKeys = Object.keys(objectValues);
              const bodyValues = Object.values(objectValues);
              const header = [
                indexKey,
                ...(properties || [
                  ...headerKeys,
                  !isMap && values.length > 0 && valuesKey,
                ]),
              ].filter(Boolean);
              const body = [indexKeys, ...bodyValues, values];
              toTable(header, body);
            };
            this.time = (label = "default") => {
              label = String(label);
              if (timerMap.has(label)) {
                this.warn(`Timer '${label}' already exists`);
                return;
              }
              timerMap.set(label, Date.now());
            };
            this.timeLog = (label = "default", ...args) => {
              label = String(label);
              if (!timerMap.has(label)) {
                this.warn(`Timer '${label}' does not exists`);
                return;
              }
              const startTime = timerMap.get(label);
              const duration = Date.now() - startTime;
              this.info(`${label}: ${duration}ms`, ...args);
            };
            this.timeEnd = (label = "default") => {
              label = String(label);
              if (!timerMap.has(label)) {
                this.warn(`Timer '${label}' does not exists`);
                return;
              }
              const startTime = timerMap.get(label);
              timerMap.delete(label);
              const duration = Date.now() - startTime;
              this.info(`${label}: ${duration}ms`);
            };
            this.group = (...label) => {
              if (label.length > 0) {
                this.log(...label);
              }
              this.indentLevel += 2;
            };
            this.groupCollapsed = this.group;
            this.groupEnd = () => {
              if (this.indentLevel > 0) {
                this.indentLevel -= 2;
              }
            };
            this.clear = () => {
              this.indentLevel = 0;
              cursorTo(files_ts_1.stdout, 0, 0);
              clearScreenDown(files_ts_1.stdout);
            };
            this.trace = (...args) => {
              const message = stringifyArgs(args, { indentLevel: 0 });
              const err = {
                name: "Trace",
                message,
              };
              // @ts-ignore
              Error.captureStackTrace(err, this.trace);
              this.error(err.stack);
            };
            this.#printFunc = printFunc;
            this.indentLevel = 0;
            this[isConsoleInstance] = true;
            // ref https://console.spec.whatwg.org/#console-namespace
            // For historical web-compatibility reasons, the namespace object for
            // console must have as its [[Prototype]] an empty object, created as if
            // by ObjectCreate(%ObjectPrototype%), instead of %ObjectPrototype%.
            const console = Object.create({});
            Object.assign(console, this);
            return console;
          }
          #printFunc;
          static [((_a = isConsoleInstance), Symbol.hasInstance)](instance) {
            return instance[isConsoleInstance];
          }
        };
        exports_24("Console", Console);
        exports_24(
          "customInspect",
          (customInspect = Symbol.for("Deno.customInspect"))
        );
        // Expose these fields to internalObject for tests.
        internals_ts_1.exposeForTest("Console", Console);
        internals_ts_1.exposeForTest("stringifyArgs", stringifyArgs);
      },
    };
  }
);
System.register(
  "$deno$/ops/fs/copy_file.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_25, context_25) {
    "use strict";
    let dispatch_json_ts_6;
    const __moduleName = context_25 && context_25.id;
    function copyFileSync(fromPath, toPath) {
      dispatch_json_ts_6.sendSync("op_copy_file", {
        from: fromPath,
        to: toPath,
      });
    }
    exports_25("copyFileSync", copyFileSync);
    async function copyFile(fromPath, toPath) {
      await dispatch_json_ts_6.sendAsync("op_copy_file", {
        from: fromPath,
        to: toPath,
      });
    }
    exports_25("copyFile", copyFile);
    return {
      setters: [
        function (dispatch_json_ts_6_1) {
          dispatch_json_ts_6 = dispatch_json_ts_6_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/dir.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_26, context_26) {
    "use strict";
    let dispatch_json_ts_7;
    const __moduleName = context_26 && context_26.id;
    function cwd() {
      return dispatch_json_ts_7.sendSync("op_cwd");
    }
    exports_26("cwd", cwd);
    function chdir(directory) {
      dispatch_json_ts_7.sendSync("op_chdir", { directory });
    }
    exports_26("chdir", chdir);
    return {
      setters: [
        function (dispatch_json_ts_7_1) {
          dispatch_json_ts_7 = dispatch_json_ts_7_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/errors.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_27, context_27) {
    "use strict";
    let dispatch_json_ts_8;
    const __moduleName = context_27 && context_27.id;
    function formatDiagnostics(items) {
      return dispatch_json_ts_8.sendSync("op_format_diagnostic", { items });
    }
    exports_27("formatDiagnostics", formatDiagnostics);
    function applySourceMap(location) {
      const { fileName, lineNumber, columnNumber } = location;
      const res = dispatch_json_ts_8.sendSync("op_apply_source_map", {
        fileName,
        lineNumber: lineNumber,
        columnNumber: columnNumber,
      });
      return {
        fileName: res.fileName,
        lineNumber: res.lineNumber,
        columnNumber: res.columnNumber,
      };
    }
    exports_27("applySourceMap", applySourceMap);
    return {
      setters: [
        function (dispatch_json_ts_8_1) {
          dispatch_json_ts_8 = dispatch_json_ts_8_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/stat.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/file_info.ts"],
  function (exports_28, context_28) {
    "use strict";
    let dispatch_json_ts_9, file_info_ts_1;
    const __moduleName = context_28 && context_28.id;
    async function lstat(path) {
      const res = await dispatch_json_ts_9.sendAsync("op_stat", {
        path,
        lstat: true,
      });
      return new file_info_ts_1.FileInfoImpl(res);
    }
    exports_28("lstat", lstat);
    function lstatSync(path) {
      const res = dispatch_json_ts_9.sendSync("op_stat", {
        path,
        lstat: true,
      });
      return new file_info_ts_1.FileInfoImpl(res);
    }
    exports_28("lstatSync", lstatSync);
    async function stat(path) {
      const res = await dispatch_json_ts_9.sendAsync("op_stat", {
        path,
        lstat: false,
      });
      return new file_info_ts_1.FileInfoImpl(res);
    }
    exports_28("stat", stat);
    function statSync(path) {
      const res = dispatch_json_ts_9.sendSync("op_stat", {
        path,
        lstat: false,
      });
      return new file_info_ts_1.FileInfoImpl(res);
    }
    exports_28("statSync", statSync);
    return {
      setters: [
        function (dispatch_json_ts_9_1) {
          dispatch_json_ts_9 = dispatch_json_ts_9_1;
        },
        function (file_info_ts_1_1) {
          file_info_ts_1 = file_info_ts_1_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register("$deno$/file_info.ts", ["$deno$/build.ts"], function (
  exports_29,
  context_29
) {
  "use strict";
  let build_ts_1, FileInfoImpl;
  const __moduleName = context_29 && context_29.id;
  return {
    setters: [
      function (build_ts_1_1) {
        build_ts_1 = build_ts_1_1;
      },
    ],
    execute: function () {
      // @internal
      FileInfoImpl = class FileInfoImpl {
        /* @internal */
        constructor(res) {
          const isUnix =
            build_ts_1.build.os === "mac" || build_ts_1.build.os === "linux";
          const modified = res.modified;
          const accessed = res.accessed;
          const created = res.created;
          const name = res.name;
          // Unix only
          const {
            dev,
            ino,
            mode,
            nlink,
            uid,
            gid,
            rdev,
            blksize,
            blocks,
          } = res;
          this.#isFile = res.isFile;
          this.#isDirectory = res.isDirectory;
          this.#isSymlink = res.isSymlink;
          this.size = res.size;
          this.modified = modified ? modified : null;
          this.accessed = accessed ? accessed : null;
          this.created = created ? created : null;
          this.name = name ? name : null;
          // Only non-null if on Unix
          this.dev = isUnix ? dev : null;
          this.ino = isUnix ? ino : null;
          this.mode = isUnix ? mode : null;
          this.nlink = isUnix ? nlink : null;
          this.uid = isUnix ? uid : null;
          this.gid = isUnix ? gid : null;
          this.rdev = isUnix ? rdev : null;
          this.blksize = isUnix ? blksize : null;
          this.blocks = isUnix ? blocks : null;
        }
        #isFile;
        #isDirectory;
        #isSymlink;
        isFile() {
          return this.#isFile;
        }
        isDirectory() {
          return this.#isDirectory;
        }
        isSymlink() {
          return this.#isSymlink;
        }
      };
      exports_29("FileInfoImpl", FileInfoImpl);
    },
  };
});
System.register(
  "$deno$/ops/fs_events.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/ops/resources.ts"],
  function (exports_30, context_30) {
    "use strict";
    let dispatch_json_ts_10, resources_ts_2, FsEvents;
    const __moduleName = context_30 && context_30.id;
    function fsEvents(paths, options = { recursive: true }) {
      return new FsEvents(Array.isArray(paths) ? paths : [paths], options);
    }
    exports_30("fsEvents", fsEvents);
    return {
      setters: [
        function (dispatch_json_ts_10_1) {
          dispatch_json_ts_10 = dispatch_json_ts_10_1;
        },
        function (resources_ts_2_1) {
          resources_ts_2 = resources_ts_2_1;
        },
      ],
      execute: function () {
        FsEvents = class FsEvents {
          constructor(paths, options) {
            const { recursive } = options;
            this.rid = dispatch_json_ts_10.sendSync("op_fs_events_open", {
              recursive,
              paths,
            });
          }
          next() {
            return dispatch_json_ts_10.sendAsync("op_fs_events_poll", {
              rid: this.rid,
            });
          }
          return(value) {
            resources_ts_2.close(this.rid);
            return Promise.resolve({ value, done: true });
          }
          [Symbol.asyncIterator]() {
            return this;
          }
        };
      },
    };
  }
);
System.register(
  "$deno$/ops/fs/link.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_31, context_31) {
    "use strict";
    let dispatch_json_ts_11;
    const __moduleName = context_31 && context_31.id;
    function linkSync(oldpath, newpath) {
      dispatch_json_ts_11.sendSync("op_link", { oldpath, newpath });
    }
    exports_31("linkSync", linkSync);
    async function link(oldpath, newpath) {
      await dispatch_json_ts_11.sendAsync("op_link", { oldpath, newpath });
    }
    exports_31("link", link);
    return {
      setters: [
        function (dispatch_json_ts_11_1) {
          dispatch_json_ts_11 = dispatch_json_ts_11_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/make_temp.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_32, context_32) {
    "use strict";
    let dispatch_json_ts_12;
    const __moduleName = context_32 && context_32.id;
    function makeTempDirSync(options = {}) {
      return dispatch_json_ts_12.sendSync("op_make_temp_dir", options);
    }
    exports_32("makeTempDirSync", makeTempDirSync);
    function makeTempDir(options = {}) {
      return dispatch_json_ts_12.sendAsync("op_make_temp_dir", options);
    }
    exports_32("makeTempDir", makeTempDir);
    function makeTempFileSync(options = {}) {
      return dispatch_json_ts_12.sendSync("op_make_temp_file", options);
    }
    exports_32("makeTempFileSync", makeTempFileSync);
    function makeTempFile(options = {}) {
      return dispatch_json_ts_12.sendAsync("op_make_temp_file", options);
    }
    exports_32("makeTempFile", makeTempFile);
    return {
      setters: [
        function (dispatch_json_ts_12_1) {
          dispatch_json_ts_12 = dispatch_json_ts_12_1;
        },
      ],
      execute: function () {},
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/runtime.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_33, context_33) {
    "use strict";
    let dispatch_json_ts_13;
    const __moduleName = context_33 && context_33.id;
    function start() {
      return dispatch_json_ts_13.sendSync("op_start");
    }
    exports_33("start", start);
    function metrics() {
      return dispatch_json_ts_13.sendSync("op_metrics");
    }
    exports_33("metrics", metrics);
    return {
      setters: [
        function (dispatch_json_ts_13_1) {
          dispatch_json_ts_13 = dispatch_json_ts_13_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/mkdir.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_34, context_34) {
    "use strict";
    let dispatch_json_ts_14;
    const __moduleName = context_34 && context_34.id;
    function mkdirArgs(path, options) {
      const args = { path, recursive: false };
      if (options) {
        if (typeof options.recursive == "boolean") {
          args.recursive = options.recursive;
        }
        if (options.mode) {
          args.mode = options.mode;
        }
      }
      return args;
    }
    function mkdirSync(path, options) {
      dispatch_json_ts_14.sendSync("op_mkdir", mkdirArgs(path, options));
    }
    exports_34("mkdirSync", mkdirSync);
    async function mkdir(path, options) {
      await dispatch_json_ts_14.sendAsync("op_mkdir", mkdirArgs(path, options));
    }
    exports_34("mkdir", mkdir);
    return {
      setters: [
        function (dispatch_json_ts_14_1) {
          dispatch_json_ts_14 = dispatch_json_ts_14_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register("$deno$/ops/net.ts", ["$deno$/ops/dispatch_json.ts"], function (
  exports_35,
  context_35
) {
  "use strict";
  let dispatch_json_ts_15, ShutdownMode;
  const __moduleName = context_35 && context_35.id;
  function shutdown(rid, how) {
    dispatch_json_ts_15.sendSync("op_shutdown", { rid, how });
  }
  exports_35("shutdown", shutdown);
  function accept(rid, transport) {
    return dispatch_json_ts_15.sendAsync("op_accept", { rid, transport });
  }
  exports_35("accept", accept);
  function listen(args) {
    return dispatch_json_ts_15.sendSync("op_listen", args);
  }
  exports_35("listen", listen);
  function connect(args) {
    return dispatch_json_ts_15.sendAsync("op_connect", args);
  }
  exports_35("connect", connect);
  function receive(rid, transport, zeroCopy) {
    return dispatch_json_ts_15.sendAsync(
      "op_receive",
      { rid, transport },
      zeroCopy
    );
  }
  exports_35("receive", receive);
  async function send(args, zeroCopy) {
    await dispatch_json_ts_15.sendAsync("op_send", args, zeroCopy);
  }
  exports_35("send", send);
  return {
    setters: [
      function (dispatch_json_ts_15_1) {
        dispatch_json_ts_15 = dispatch_json_ts_15_1;
      },
    ],
    execute: function () {
      (function (ShutdownMode) {
        // See http://man7.org/linux/man-pages/man2/shutdown.2.html
        // Corresponding to SHUT_RD, SHUT_WR, SHUT_RDWR
        ShutdownMode[(ShutdownMode["Read"] = 0)] = "Read";
        ShutdownMode[(ShutdownMode["Write"] = 1)] = "Write";
        ShutdownMode[(ShutdownMode["ReadWrite"] = 2)] = "ReadWrite";
      })(ShutdownMode || (ShutdownMode = {}));
      exports_35("ShutdownMode", ShutdownMode);
    },
  };
});
System.register(
  "$deno$/net.ts",
  [
    "$deno$/errors.ts",
    "$deno$/ops/io.ts",
    "$deno$/ops/resources.ts",
    "$deno$/ops/net.ts",
  ],
  function (exports_36, context_36) {
    "use strict";
    let errors_ts_2,
      io_ts_4,
      resources_ts_3,
      netOps,
      ConnImpl,
      ListenerImpl,
      DatagramImpl;
    const __moduleName = context_36 && context_36.id;
    function listen(options) {
      let res;
      if (options.transport === "unix" || options.transport === "unixpacket") {
        res = netOps.listen(options);
      } else {
        res = netOps.listen({
          transport: "tcp",
          hostname: "127.0.0.1",
          ...options,
        });
      }
      if (
        !options.transport ||
        options.transport === "tcp" ||
        options.transport === "unix"
      ) {
        return new ListenerImpl(res.rid, res.localAddr);
      } else {
        return new DatagramImpl(res.rid, res.localAddr);
      }
    }
    exports_36("listen", listen);
    async function connect(options) {
      let res;
      if (options.transport === "unix") {
        res = await netOps.connect(options);
      } else {
        res = await netOps.connect({
          transport: "tcp",
          hostname: "127.0.0.1",
          ...options,
        });
      }
      return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
    }
    exports_36("connect", connect);
    return {
      setters: [
        function (errors_ts_2_1) {
          errors_ts_2 = errors_ts_2_1;
        },
        function (io_ts_4_1) {
          io_ts_4 = io_ts_4_1;
        },
        function (resources_ts_3_1) {
          resources_ts_3 = resources_ts_3_1;
        },
        function (netOps_1) {
          netOps = netOps_1;
          exports_36({
            ShutdownMode: netOps_1["ShutdownMode"],
            shutdown: netOps_1["shutdown"],
          });
        },
      ],
      execute: function () {
        ConnImpl = class ConnImpl {
          constructor(rid, remoteAddr, localAddr) {
            this.rid = rid;
            this.remoteAddr = remoteAddr;
            this.localAddr = localAddr;
          }
          write(p) {
            return io_ts_4.write(this.rid, p);
          }
          read(p) {
            return io_ts_4.read(this.rid, p);
          }
          close() {
            resources_ts_3.close(this.rid);
          }
          closeRead() {
            netOps.shutdown(this.rid, netOps.ShutdownMode.Read);
          }
          closeWrite() {
            netOps.shutdown(this.rid, netOps.ShutdownMode.Write);
          }
        };
        exports_36("ConnImpl", ConnImpl);
        ListenerImpl = class ListenerImpl {
          constructor(rid, addr) {
            this.rid = rid;
            this.addr = addr;
          }
          async accept() {
            const res = await netOps.accept(this.rid, this.addr.transport);
            return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
          }
          close() {
            resources_ts_3.close(this.rid);
          }
          async *[Symbol.asyncIterator]() {
            while (true) {
              try {
                yield await this.accept();
              } catch (error) {
                if (error instanceof errors_ts_2.errors.BadResource) {
                  break;
                }
                throw error;
              }
            }
          }
        };
        exports_36("ListenerImpl", ListenerImpl);
        DatagramImpl = class DatagramImpl {
          constructor(rid, addr, bufSize = 1024) {
            this.rid = rid;
            this.addr = addr;
            this.bufSize = bufSize;
          }
          async receive(p) {
            const buf = p || new Uint8Array(this.bufSize);
            const { size, remoteAddr } = await netOps.receive(
              this.rid,
              this.addr.transport,
              buf
            );
            const sub = buf.subarray(0, size);
            return [sub, remoteAddr];
          }
          async send(p, addr) {
            const remote = { hostname: "127.0.0.1", transport: "udp", ...addr };
            const args = { ...remote, rid: this.rid };
            await netOps.send(args, p);
          }
          close() {
            resources_ts_3.close(this.rid);
          }
          async *[Symbol.asyncIterator]() {
            while (true) {
              try {
                yield await this.receive();
              } catch (error) {
                if (error instanceof errors_ts_2.errors.BadResource) {
                  break;
                }
                throw error;
              }
            }
          }
        };
        exports_36("DatagramImpl", DatagramImpl);
      },
    };
  }
);
System.register(
  "$deno$/ops/os.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/errors.ts"],
  function (exports_37, context_37) {
    "use strict";
    let dispatch_json_ts_16, errors_ts_3;
    const __moduleName = context_37 && context_37.id;
    function loadavg() {
      return dispatch_json_ts_16.sendSync("op_loadavg");
    }
    exports_37("loadavg", loadavg);
    function hostname() {
      return dispatch_json_ts_16.sendSync("op_hostname");
    }
    exports_37("hostname", hostname);
    function osRelease() {
      return dispatch_json_ts_16.sendSync("op_os_release");
    }
    exports_37("osRelease", osRelease);
    function exit(code = 0) {
      dispatch_json_ts_16.sendSync("op_exit", { code });
      throw new Error("Code not reachable");
    }
    exports_37("exit", exit);
    function setEnv(key, value) {
      dispatch_json_ts_16.sendSync("op_set_env", { key, value });
    }
    function getEnv(key) {
      return dispatch_json_ts_16.sendSync("op_get_env", { key })[0];
    }
    function env(key) {
      if (key) {
        return getEnv(key);
      }
      const env = dispatch_json_ts_16.sendSync("op_env");
      return new Proxy(env, {
        set(obj, prop, value) {
          setEnv(prop, value);
          return Reflect.set(obj, prop, value);
        },
      });
    }
    exports_37("env", env);
    function dir(kind) {
      try {
        return dispatch_json_ts_16.sendSync("op_get_dir", { kind });
      } catch (error) {
        if (error instanceof errors_ts_3.errors.PermissionDenied) {
          throw error;
        }
        return null;
      }
    }
    exports_37("dir", dir);
    function execPath() {
      return dispatch_json_ts_16.sendSync("op_exec_path");
    }
    exports_37("execPath", execPath);
    return {
      setters: [
        function (dispatch_json_ts_16_1) {
          dispatch_json_ts_16 = dispatch_json_ts_16_1;
        },
        function (errors_ts_3_1) {
          errors_ts_3 = errors_ts_3_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/permissions.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_38, context_38) {
    "use strict";
    let dispatch_json_ts_17;
    const __moduleName = context_38 && context_38.id;
    function query(desc) {
      return dispatch_json_ts_17.sendSync("op_query_permission", desc).state;
    }
    exports_38("query", query);
    function revoke(desc) {
      return dispatch_json_ts_17.sendSync("op_revoke_permission", desc).state;
    }
    exports_38("revoke", revoke);
    function request(desc) {
      return dispatch_json_ts_17.sendSync("op_request_permission", desc).state;
    }
    exports_38("request", request);
    return {
      setters: [
        function (dispatch_json_ts_17_1) {
          dispatch_json_ts_17 = dispatch_json_ts_17_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/permissions.ts",
  ["$deno$/ops/permissions.ts"],
  function (exports_39, context_39) {
    "use strict";
    let permissionsOps, PermissionStatus, Permissions;
    const __moduleName = context_39 && context_39.id;
    return {
      setters: [
        function (permissionsOps_1) {
          permissionsOps = permissionsOps_1;
        },
      ],
      execute: function () {
        PermissionStatus = class PermissionStatus {
          constructor(state) {
            this.state = state;
          }
        };
        exports_39("PermissionStatus", PermissionStatus);
        Permissions = class Permissions {
          query(desc) {
            const state = permissionsOps.query(desc);
            return Promise.resolve(new PermissionStatus(state));
          }
          revoke(desc) {
            const state = permissionsOps.revoke(desc);
            return Promise.resolve(new PermissionStatus(state));
          }
          request(desc) {
            const state = permissionsOps.request(desc);
            return Promise.resolve(new PermissionStatus(state));
          }
        };
        exports_39("Permissions", Permissions);
        exports_39("permissions", new Permissions());
      },
    };
  }
);
System.register(
  "$deno$/ops/plugins.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_40, context_40) {
    "use strict";
    let dispatch_json_ts_18;
    const __moduleName = context_40 && context_40.id;
    function openPlugin(filename) {
      return dispatch_json_ts_18.sendSync("op_open_plugin", { filename });
    }
    exports_40("openPlugin", openPlugin);
    return {
      setters: [
        function (dispatch_json_ts_18_1) {
          dispatch_json_ts_18 = dispatch_json_ts_18_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/plugins.ts",
  ["$deno$/ops/plugins.ts", "$deno$/core.ts"],
  function (exports_41, context_41) {
    "use strict";
    let plugins_ts_1, core_ts_3, PluginOpImpl, PluginImpl;
    const __moduleName = context_41 && context_41.id;
    function openPlugin(filename) {
      const response = plugins_ts_1.openPlugin(filename);
      return new PluginImpl(response.rid, response.ops);
    }
    exports_41("openPlugin", openPlugin);
    return {
      setters: [
        function (plugins_ts_1_1) {
          plugins_ts_1 = plugins_ts_1_1;
        },
        function (core_ts_3_1) {
          core_ts_3 = core_ts_3_1;
        },
      ],
      execute: function () {
        PluginOpImpl = class PluginOpImpl {
          constructor(opId) {
            this.#opId = opId;
          }
          #opId;
          dispatch(control, zeroCopy) {
            return core_ts_3.core.dispatch(this.#opId, control, zeroCopy);
          }
          setAsyncHandler(handler) {
            core_ts_3.core.setAsyncHandler(this.#opId, handler);
          }
        };
        PluginImpl = class PluginImpl {
          constructor(_rid, ops) {
            this.#ops = {};
            for (const op in ops) {
              this.#ops[op] = new PluginOpImpl(ops[op]);
            }
          }
          #ops;
          get ops() {
            return Object.assign({}, this.#ops);
          }
        };
      },
    };
  }
);
System.register(
  "$deno$/ops/process.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/util.ts", "$deno$/build.ts"],
  function (exports_42, context_42) {
    "use strict";
    let dispatch_json_ts_19, util_ts_4, build_ts_2;
    const __moduleName = context_42 && context_42.id;
    function kill(pid, signo) {
      if (build_ts_2.build.os === "win") {
        throw new Error("Not yet implemented");
      }
      dispatch_json_ts_19.sendSync("op_kill", { pid, signo });
    }
    exports_42("kill", kill);
    function runStatus(rid) {
      return dispatch_json_ts_19.sendAsync("op_run_status", { rid });
    }
    exports_42("runStatus", runStatus);
    function run(request) {
      util_ts_4.assert(request.cmd.length > 0);
      return dispatch_json_ts_19.sendSync("op_run", request);
    }
    exports_42("run", run);
    return {
      setters: [
        function (dispatch_json_ts_19_1) {
          dispatch_json_ts_19 = dispatch_json_ts_19_1;
        },
        function (util_ts_4_1) {
          util_ts_4 = util_ts_4_1;
        },
        function (build_ts_2_1) {
          build_ts_2 = build_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/process.ts",
  [
    "$deno$/files.ts",
    "$deno$/ops/resources.ts",
    "$deno$/buffer.ts",
    "$deno$/ops/process.ts",
  ],
  function (exports_43, context_43) {
    "use strict";
    let files_ts_2, resources_ts_4, buffer_ts_1, process_ts_1, Process;
    const __moduleName = context_43 && context_43.id;
    async function runStatus(rid) {
      const res = await process_ts_1.runStatus(rid);
      if (res.gotSignal) {
        const signal = res.exitSignal;
        return { signal, success: false };
      } else {
        const code = res.exitCode;
        return { code, success: code === 0 };
      }
    }
    function isRid(arg) {
      return !isNaN(arg);
    }
    function run({
      cmd,
      cwd = undefined,
      env = {},
      stdout = "inherit",
      stderr = "inherit",
      stdin = "inherit",
    }) {
      const res = process_ts_1.run({
        cmd: cmd.map(String),
        cwd,
        env: Object.entries(env),
        stdin: isRid(stdin) ? "" : stdin,
        stdout: isRid(stdout) ? "" : stdout,
        stderr: isRid(stderr) ? "" : stderr,
        stdinRid: isRid(stdin) ? stdin : 0,
        stdoutRid: isRid(stdout) ? stdout : 0,
        stderrRid: isRid(stderr) ? stderr : 0,
      });
      return new Process(res);
    }
    exports_43("run", run);
    return {
      setters: [
        function (files_ts_2_1) {
          files_ts_2 = files_ts_2_1;
        },
        function (resources_ts_4_1) {
          resources_ts_4 = resources_ts_4_1;
        },
        function (buffer_ts_1_1) {
          buffer_ts_1 = buffer_ts_1_1;
        },
        function (process_ts_1_1) {
          process_ts_1 = process_ts_1_1;
        },
      ],
      execute: function () {
        Process = class Process {
          // @internal
          constructor(res) {
            this.rid = res.rid;
            this.pid = res.pid;
            if (res.stdinRid && res.stdinRid > 0) {
              this.stdin = new files_ts_2.File(res.stdinRid);
            }
            if (res.stdoutRid && res.stdoutRid > 0) {
              this.stdout = new files_ts_2.File(res.stdoutRid);
            }
            if (res.stderrRid && res.stderrRid > 0) {
              this.stderr = new files_ts_2.File(res.stderrRid);
            }
          }
          status() {
            return runStatus(this.rid);
          }
          async output() {
            if (!this.stdout) {
              throw new Error("Process.output: stdout is undefined");
            }
            try {
              return await buffer_ts_1.readAll(this.stdout);
            } finally {
              this.stdout.close();
            }
          }
          async stderrOutput() {
            if (!this.stderr) {
              throw new Error("Process.stderrOutput: stderr is undefined");
            }
            try {
              return await buffer_ts_1.readAll(this.stderr);
            } finally {
              this.stderr.close();
            }
          }
          close() {
            resources_ts_4.close(this.rid);
          }
          kill(signo) {
            process_ts_1.kill(this.pid, signo);
          }
        };
        exports_43("Process", Process);
      },
    };
  }
);
System.register(
  "$deno$/ops/fs/read_dir.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/file_info.ts"],
  function (exports_44, context_44) {
    "use strict";
    let dispatch_json_ts_20, file_info_ts_2;
    const __moduleName = context_44 && context_44.id;
    function res(response) {
      return response.entries.map((statRes) => {
        return new file_info_ts_2.FileInfoImpl(statRes);
      });
    }
    function readdirSync(path) {
      return res(dispatch_json_ts_20.sendSync("op_read_dir", { path }));
    }
    exports_44("readdirSync", readdirSync);
    async function readdir(path) {
      return res(await dispatch_json_ts_20.sendAsync("op_read_dir", { path }));
    }
    exports_44("readdir", readdir);
    return {
      setters: [
        function (dispatch_json_ts_20_1) {
          dispatch_json_ts_20 = dispatch_json_ts_20_1;
        },
        function (file_info_ts_2_1) {
          file_info_ts_2 = file_info_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/read_file.ts",
  ["$deno$/files.ts", "$deno$/buffer.ts"],
  function (exports_45, context_45) {
    "use strict";
    let files_ts_3, buffer_ts_2;
    const __moduleName = context_45 && context_45.id;
    function readFileSync(path) {
      const file = files_ts_3.openSync(path);
      const contents = buffer_ts_2.readAllSync(file);
      file.close();
      return contents;
    }
    exports_45("readFileSync", readFileSync);
    async function readFile(path) {
      const file = await files_ts_3.open(path);
      const contents = await buffer_ts_2.readAll(file);
      file.close();
      return contents;
    }
    exports_45("readFile", readFile);
    return {
      setters: [
        function (files_ts_3_1) {
          files_ts_3 = files_ts_3_1;
        },
        function (buffer_ts_2_1) {
          buffer_ts_2 = buffer_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/read_link.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_46, context_46) {
    "use strict";
    let dispatch_json_ts_21;
    const __moduleName = context_46 && context_46.id;
    function readlinkSync(path) {
      return dispatch_json_ts_21.sendSync("op_read_link", { path });
    }
    exports_46("readlinkSync", readlinkSync);
    function readlink(path) {
      return dispatch_json_ts_21.sendAsync("op_read_link", { path });
    }
    exports_46("readlink", readlink);
    return {
      setters: [
        function (dispatch_json_ts_21_1) {
          dispatch_json_ts_21 = dispatch_json_ts_21_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/realpath.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_47, context_47) {
    "use strict";
    let dispatch_json_ts_22;
    const __moduleName = context_47 && context_47.id;
    function realpathSync(path) {
      return dispatch_json_ts_22.sendSync("op_realpath", { path });
    }
    exports_47("realpathSync", realpathSync);
    function realpath(path) {
      return dispatch_json_ts_22.sendAsync("op_realpath", { path });
    }
    exports_47("realpath", realpath);
    return {
      setters: [
        function (dispatch_json_ts_22_1) {
          dispatch_json_ts_22 = dispatch_json_ts_22_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/remove.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_48, context_48) {
    "use strict";
    let dispatch_json_ts_23;
    const __moduleName = context_48 && context_48.id;
    function removeSync(path, options = {}) {
      dispatch_json_ts_23.sendSync("op_remove", {
        path,
        recursive: !!options.recursive,
      });
    }
    exports_48("removeSync", removeSync);
    async function remove(path, options = {}) {
      await dispatch_json_ts_23.sendAsync("op_remove", {
        path,
        recursive: !!options.recursive,
      });
    }
    exports_48("remove", remove);
    return {
      setters: [
        function (dispatch_json_ts_23_1) {
          dispatch_json_ts_23 = dispatch_json_ts_23_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/rename.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_49, context_49) {
    "use strict";
    let dispatch_json_ts_24;
    const __moduleName = context_49 && context_49.id;
    function renameSync(oldpath, newpath) {
      dispatch_json_ts_24.sendSync("op_rename", { oldpath, newpath });
    }
    exports_49("renameSync", renameSync);
    async function rename(oldpath, newpath) {
      await dispatch_json_ts_24.sendAsync("op_rename", { oldpath, newpath });
    }
    exports_49("rename", rename);
    return {
      setters: [
        function (dispatch_json_ts_24_1) {
          dispatch_json_ts_24 = dispatch_json_ts_24_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/signal.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_50, context_50) {
    "use strict";
    let dispatch_json_ts_25;
    const __moduleName = context_50 && context_50.id;
    function bindSignal(signo) {
      return dispatch_json_ts_25.sendSync("op_signal_bind", { signo });
    }
    exports_50("bindSignal", bindSignal);
    function pollSignal(rid) {
      return dispatch_json_ts_25.sendAsync("op_signal_poll", { rid });
    }
    exports_50("pollSignal", pollSignal);
    function unbindSignal(rid) {
      dispatch_json_ts_25.sendSync("op_signal_unbind", { rid });
    }
    exports_50("unbindSignal", unbindSignal);
    return {
      setters: [
        function (dispatch_json_ts_25_1) {
          dispatch_json_ts_25 = dispatch_json_ts_25_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/signals.ts",
  ["$deno$/ops/signal.ts", "$deno$/build.ts"],
  function (exports_51, context_51) {
    "use strict";
    let signal_ts_1, build_ts_3, LinuxSignal, MacOSSignal, Signal, SignalStream;
    const __moduleName = context_51 && context_51.id;
    function setSignals() {
      if (build_ts_3.build.os === "mac") {
        Object.assign(Signal, MacOSSignal);
      } else {
        Object.assign(Signal, LinuxSignal);
      }
    }
    exports_51("setSignals", setSignals);
    function signal(signo) {
      if (build_ts_3.build.os === "win") {
        throw new Error("not implemented!");
      }
      return new SignalStream(signo);
    }
    exports_51("signal", signal);
    return {
      setters: [
        function (signal_ts_1_1) {
          signal_ts_1 = signal_ts_1_1;
        },
        function (build_ts_3_1) {
          build_ts_3 = build_ts_3_1;
        },
      ],
      execute: function () {
        // From `kill -l`
        (function (LinuxSignal) {
          LinuxSignal[(LinuxSignal["SIGHUP"] = 1)] = "SIGHUP";
          LinuxSignal[(LinuxSignal["SIGINT"] = 2)] = "SIGINT";
          LinuxSignal[(LinuxSignal["SIGQUIT"] = 3)] = "SIGQUIT";
          LinuxSignal[(LinuxSignal["SIGILL"] = 4)] = "SIGILL";
          LinuxSignal[(LinuxSignal["SIGTRAP"] = 5)] = "SIGTRAP";
          LinuxSignal[(LinuxSignal["SIGABRT"] = 6)] = "SIGABRT";
          LinuxSignal[(LinuxSignal["SIGBUS"] = 7)] = "SIGBUS";
          LinuxSignal[(LinuxSignal["SIGFPE"] = 8)] = "SIGFPE";
          LinuxSignal[(LinuxSignal["SIGKILL"] = 9)] = "SIGKILL";
          LinuxSignal[(LinuxSignal["SIGUSR1"] = 10)] = "SIGUSR1";
          LinuxSignal[(LinuxSignal["SIGSEGV"] = 11)] = "SIGSEGV";
          LinuxSignal[(LinuxSignal["SIGUSR2"] = 12)] = "SIGUSR2";
          LinuxSignal[(LinuxSignal["SIGPIPE"] = 13)] = "SIGPIPE";
          LinuxSignal[(LinuxSignal["SIGALRM"] = 14)] = "SIGALRM";
          LinuxSignal[(LinuxSignal["SIGTERM"] = 15)] = "SIGTERM";
          LinuxSignal[(LinuxSignal["SIGSTKFLT"] = 16)] = "SIGSTKFLT";
          LinuxSignal[(LinuxSignal["SIGCHLD"] = 17)] = "SIGCHLD";
          LinuxSignal[(LinuxSignal["SIGCONT"] = 18)] = "SIGCONT";
          LinuxSignal[(LinuxSignal["SIGSTOP"] = 19)] = "SIGSTOP";
          LinuxSignal[(LinuxSignal["SIGTSTP"] = 20)] = "SIGTSTP";
          LinuxSignal[(LinuxSignal["SIGTTIN"] = 21)] = "SIGTTIN";
          LinuxSignal[(LinuxSignal["SIGTTOU"] = 22)] = "SIGTTOU";
          LinuxSignal[(LinuxSignal["SIGURG"] = 23)] = "SIGURG";
          LinuxSignal[(LinuxSignal["SIGXCPU"] = 24)] = "SIGXCPU";
          LinuxSignal[(LinuxSignal["SIGXFSZ"] = 25)] = "SIGXFSZ";
          LinuxSignal[(LinuxSignal["SIGVTALRM"] = 26)] = "SIGVTALRM";
          LinuxSignal[(LinuxSignal["SIGPROF"] = 27)] = "SIGPROF";
          LinuxSignal[(LinuxSignal["SIGWINCH"] = 28)] = "SIGWINCH";
          LinuxSignal[(LinuxSignal["SIGIO"] = 29)] = "SIGIO";
          LinuxSignal[(LinuxSignal["SIGPWR"] = 30)] = "SIGPWR";
          LinuxSignal[(LinuxSignal["SIGSYS"] = 31)] = "SIGSYS";
        })(LinuxSignal || (LinuxSignal = {}));
        // From `kill -l`
        (function (MacOSSignal) {
          MacOSSignal[(MacOSSignal["SIGHUP"] = 1)] = "SIGHUP";
          MacOSSignal[(MacOSSignal["SIGINT"] = 2)] = "SIGINT";
          MacOSSignal[(MacOSSignal["SIGQUIT"] = 3)] = "SIGQUIT";
          MacOSSignal[(MacOSSignal["SIGILL"] = 4)] = "SIGILL";
          MacOSSignal[(MacOSSignal["SIGTRAP"] = 5)] = "SIGTRAP";
          MacOSSignal[(MacOSSignal["SIGABRT"] = 6)] = "SIGABRT";
          MacOSSignal[(MacOSSignal["SIGEMT"] = 7)] = "SIGEMT";
          MacOSSignal[(MacOSSignal["SIGFPE"] = 8)] = "SIGFPE";
          MacOSSignal[(MacOSSignal["SIGKILL"] = 9)] = "SIGKILL";
          MacOSSignal[(MacOSSignal["SIGBUS"] = 10)] = "SIGBUS";
          MacOSSignal[(MacOSSignal["SIGSEGV"] = 11)] = "SIGSEGV";
          MacOSSignal[(MacOSSignal["SIGSYS"] = 12)] = "SIGSYS";
          MacOSSignal[(MacOSSignal["SIGPIPE"] = 13)] = "SIGPIPE";
          MacOSSignal[(MacOSSignal["SIGALRM"] = 14)] = "SIGALRM";
          MacOSSignal[(MacOSSignal["SIGTERM"] = 15)] = "SIGTERM";
          MacOSSignal[(MacOSSignal["SIGURG"] = 16)] = "SIGURG";
          MacOSSignal[(MacOSSignal["SIGSTOP"] = 17)] = "SIGSTOP";
          MacOSSignal[(MacOSSignal["SIGTSTP"] = 18)] = "SIGTSTP";
          MacOSSignal[(MacOSSignal["SIGCONT"] = 19)] = "SIGCONT";
          MacOSSignal[(MacOSSignal["SIGCHLD"] = 20)] = "SIGCHLD";
          MacOSSignal[(MacOSSignal["SIGTTIN"] = 21)] = "SIGTTIN";
          MacOSSignal[(MacOSSignal["SIGTTOU"] = 22)] = "SIGTTOU";
          MacOSSignal[(MacOSSignal["SIGIO"] = 23)] = "SIGIO";
          MacOSSignal[(MacOSSignal["SIGXCPU"] = 24)] = "SIGXCPU";
          MacOSSignal[(MacOSSignal["SIGXFSZ"] = 25)] = "SIGXFSZ";
          MacOSSignal[(MacOSSignal["SIGVTALRM"] = 26)] = "SIGVTALRM";
          MacOSSignal[(MacOSSignal["SIGPROF"] = 27)] = "SIGPROF";
          MacOSSignal[(MacOSSignal["SIGWINCH"] = 28)] = "SIGWINCH";
          MacOSSignal[(MacOSSignal["SIGINFO"] = 29)] = "SIGINFO";
          MacOSSignal[(MacOSSignal["SIGUSR1"] = 30)] = "SIGUSR1";
          MacOSSignal[(MacOSSignal["SIGUSR2"] = 31)] = "SIGUSR2";
        })(MacOSSignal || (MacOSSignal = {}));
        exports_51("Signal", (Signal = {}));
        exports_51("signals", {
          alarm() {
            return signal(Signal.SIGALRM);
          },
          child() {
            return signal(Signal.SIGCHLD);
          },
          hungup() {
            return signal(Signal.SIGHUP);
          },
          interrupt() {
            return signal(Signal.SIGINT);
          },
          io() {
            return signal(Signal.SIGIO);
          },
          pipe() {
            return signal(Signal.SIGPIPE);
          },
          quit() {
            return signal(Signal.SIGQUIT);
          },
          terminate() {
            return signal(Signal.SIGTERM);
          },
          userDefined1() {
            return signal(Signal.SIGUSR1);
          },
          userDefined2() {
            return signal(Signal.SIGUSR2);
          },
          windowChange() {
            return signal(Signal.SIGWINCH);
          },
        });
        SignalStream = class SignalStream {
          constructor(signo) {
            this.#disposed = false;
            this.#pollingPromise = Promise.resolve(false);
            this.#pollSignal = async () => {
              const res = await signal_ts_1.pollSignal(this.#rid);
              return res.done;
            };
            this.#loop = async () => {
              do {
                this.#pollingPromise = this.#pollSignal();
              } while (!(await this.#pollingPromise) && !this.#disposed);
            };
            this.#rid = signal_ts_1.bindSignal(signo).rid;
            this.#loop();
          }
          #disposed;
          #pollingPromise;
          #rid;
          #pollSignal;
          #loop;
          then(f, g) {
            return this.#pollingPromise.then(() => {}).then(f, g);
          }
          async next() {
            return { done: await this.#pollingPromise, value: undefined };
          }
          [Symbol.asyncIterator]() {
            return this;
          }
          dispose() {
            if (this.#disposed) {
              throw new Error("The stream has already been disposed.");
            }
            this.#disposed = true;
            signal_ts_1.unbindSignal(this.#rid);
          }
        };
        exports_51("SignalStream", SignalStream);
      },
    };
  }
);
System.register(
  "$deno$/ops/fs/symlink.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/util.ts", "$deno$/build.ts"],
  function (exports_52, context_52) {
    "use strict";
    let dispatch_json_ts_26, util, build_ts_4;
    const __moduleName = context_52 && context_52.id;
    function symlinkSync(oldpath, newpath, type) {
      if (build_ts_4.build.os === "win" && type) {
        return util.notImplemented();
      }
      dispatch_json_ts_26.sendSync("op_symlink", { oldpath, newpath });
    }
    exports_52("symlinkSync", symlinkSync);
    async function symlink(oldpath, newpath, type) {
      if (build_ts_4.build.os === "win" && type) {
        return util.notImplemented();
      }
      await dispatch_json_ts_26.sendAsync("op_symlink", { oldpath, newpath });
    }
    exports_52("symlink", symlink);
    return {
      setters: [
        function (dispatch_json_ts_26_1) {
          dispatch_json_ts_26 = dispatch_json_ts_26_1;
        },
        function (util_2) {
          util = util_2;
        },
        function (build_ts_4_1) {
          build_ts_4 = build_ts_4_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register("$deno$/ops/tls.ts", ["$deno$/ops/dispatch_json.ts"], function (
  exports_53,
  context_53
) {
  "use strict";
  let dispatch_json_ts_27;
  const __moduleName = context_53 && context_53.id;
  function connectTLS(args) {
    return dispatch_json_ts_27.sendAsync("op_connect_tls", args);
  }
  exports_53("connectTLS", connectTLS);
  function acceptTLS(rid) {
    return dispatch_json_ts_27.sendAsync("op_accept_tls", { rid });
  }
  exports_53("acceptTLS", acceptTLS);
  function listenTLS(args) {
    return dispatch_json_ts_27.sendSync("op_listen_tls", args);
  }
  exports_53("listenTLS", listenTLS);
  return {
    setters: [
      function (dispatch_json_ts_27_1) {
        dispatch_json_ts_27 = dispatch_json_ts_27_1;
      },
    ],
    execute: function () {},
  };
});
System.register(
  "$deno$/tls.ts",
  ["$deno$/ops/tls.ts", "$deno$/net.ts"],
  function (exports_54, context_54) {
    "use strict";
    let tlsOps, net_ts_1, TLSListenerImpl;
    const __moduleName = context_54 && context_54.id;
    async function connectTLS({
      port,
      hostname = "127.0.0.1",
      transport = "tcp",
      certFile = undefined,
    }) {
      const res = await tlsOps.connectTLS({
        port,
        hostname,
        transport,
        certFile,
      });
      return new net_ts_1.ConnImpl(res.rid, res.remoteAddr, res.localAddr);
    }
    exports_54("connectTLS", connectTLS);
    function listenTLS({
      port,
      certFile,
      keyFile,
      hostname = "0.0.0.0",
      transport = "tcp",
    }) {
      const res = tlsOps.listenTLS({
        port,
        certFile,
        keyFile,
        hostname,
        transport,
      });
      return new TLSListenerImpl(res.rid, res.localAddr);
    }
    exports_54("listenTLS", listenTLS);
    return {
      setters: [
        function (tlsOps_1) {
          tlsOps = tlsOps_1;
        },
        function (net_ts_1_1) {
          net_ts_1 = net_ts_1_1;
        },
      ],
      execute: function () {
        TLSListenerImpl = class TLSListenerImpl extends net_ts_1.ListenerImpl {
          async accept() {
            const res = await tlsOps.acceptTLS(this.rid);
            return new net_ts_1.ConnImpl(
              res.rid,
              res.remoteAddr,
              res.localAddr
            );
          }
        };
      },
    };
  }
);
System.register(
  "$deno$/ops/fs/truncate.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_55, context_55) {
    "use strict";
    let dispatch_json_ts_28;
    const __moduleName = context_55 && context_55.id;
    function coerceLen(len) {
      if (!len) {
        return 0;
      }
      if (len < 0) {
        return 0;
      }
      return len;
    }
    function truncateSync(path, len) {
      dispatch_json_ts_28.sendSync("op_truncate", {
        path,
        len: coerceLen(len),
      });
    }
    exports_55("truncateSync", truncateSync);
    async function truncate(path, len) {
      await dispatch_json_ts_28.sendAsync("op_truncate", {
        path,
        len: coerceLen(len),
      });
    }
    exports_55("truncate", truncate);
    return {
      setters: [
        function (dispatch_json_ts_28_1) {
          dispatch_json_ts_28 = dispatch_json_ts_28_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register("$deno$/ops/tty.ts", ["$deno$/ops/dispatch_json.ts"], function (
  exports_56,
  context_56
) {
  "use strict";
  let dispatch_json_ts_29;
  const __moduleName = context_56 && context_56.id;
  function isatty(rid) {
    return dispatch_json_ts_29.sendSync("op_isatty", { rid });
  }
  exports_56("isatty", isatty);
  function setRaw(rid, mode) {
    dispatch_json_ts_29.sendSync("op_set_raw", {
      rid,
      mode,
    });
  }
  exports_56("setRaw", setRaw);
  return {
    setters: [
      function (dispatch_json_ts_29_1) {
        dispatch_json_ts_29 = dispatch_json_ts_29_1;
      },
    ],
    execute: function () {},
  };
});
System.register(
  "$deno$/ops/fs/umask.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_57, context_57) {
    "use strict";
    let dispatch_json_ts_30;
    const __moduleName = context_57 && context_57.id;
    function umask(mask) {
      return dispatch_json_ts_30.sendSync("op_umask", { mask });
    }
    exports_57("umask", umask);
    return {
      setters: [
        function (dispatch_json_ts_30_1) {
          dispatch_json_ts_30 = dispatch_json_ts_30_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/fs/utime.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_58, context_58) {
    "use strict";
    let dispatch_json_ts_31;
    const __moduleName = context_58 && context_58.id;
    function toSecondsFromEpoch(v) {
      return v instanceof Date ? Math.trunc(v.valueOf() / 1000) : v;
    }
    function utimeSync(path, atime, mtime) {
      dispatch_json_ts_31.sendSync("op_utime", {
        path,
        // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
        atime: toSecondsFromEpoch(atime),
        mtime: toSecondsFromEpoch(mtime),
      });
    }
    exports_58("utimeSync", utimeSync);
    async function utime(path, atime, mtime) {
      await dispatch_json_ts_31.sendAsync("op_utime", {
        path,
        // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
        atime: toSecondsFromEpoch(atime),
        mtime: toSecondsFromEpoch(mtime),
      });
    }
    exports_58("utime", utime);
    return {
      setters: [
        function (dispatch_json_ts_31_1) {
          dispatch_json_ts_31 = dispatch_json_ts_31_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/write_file.ts",
  [
    "$deno$/ops/fs/stat.ts",
    "$deno$/files.ts",
    "$deno$/ops/fs/chmod.ts",
    "$deno$/buffer.ts",
    "$deno$/build.ts",
  ],
  function (exports_59, context_59) {
    "use strict";
    let stat_ts_1, files_ts_4, chmod_ts_1, buffer_ts_3, build_ts_5;
    const __moduleName = context_59 && context_59.id;
    function writeFileSync(path, data, options = {}) {
      if (options.create !== undefined) {
        const create = !!options.create;
        if (!create) {
          // verify that file exists
          stat_ts_1.statSync(path);
        }
      }
      const openMode = !!options.append ? "a" : "w";
      const file = files_ts_4.openSync(path, openMode);
      if (
        options.mode !== undefined &&
        options.mode !== null &&
        build_ts_5.build.os !== "win"
      ) {
        chmod_ts_1.chmodSync(path, options.mode);
      }
      buffer_ts_3.writeAllSync(file, data);
      file.close();
    }
    exports_59("writeFileSync", writeFileSync);
    async function writeFile(path, data, options = {}) {
      if (options.create !== undefined) {
        const create = !!options.create;
        if (!create) {
          // verify that file exists
          await stat_ts_1.stat(path);
        }
      }
      const openMode = !!options.append ? "a" : "w";
      const file = await files_ts_4.open(path, openMode);
      if (
        options.mode !== undefined &&
        options.mode !== null &&
        build_ts_5.build.os !== "win"
      ) {
        await chmod_ts_1.chmod(path, options.mode);
      }
      await buffer_ts_3.writeAll(file, data);
      file.close();
    }
    exports_59("writeFile", writeFile);
    return {
      setters: [
        function (stat_ts_1_1) {
          stat_ts_1 = stat_ts_1_1;
        },
        function (files_ts_4_1) {
          files_ts_4 = files_ts_4_1;
        },
        function (chmod_ts_1_1) {
          chmod_ts_1 = chmod_ts_1_1;
        },
        function (buffer_ts_3_1) {
          buffer_ts_3 = buffer_ts_3_1;
        },
        function (build_ts_5_1) {
          build_ts_5 = build_ts_5_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/testing.ts",
  [
    "$deno$/colors.ts",
    "$deno$/ops/os.ts",
    "$deno$/web/console.ts",
    "$deno$/files.ts",
    "$deno$/internals.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/ops/runtime.ts",
    "$deno$/ops/resources.ts",
    "$deno$/util.ts",
  ],
  function (exports_60, context_60) {
    "use strict";
    let colors_ts_1,
      os_ts_1,
      console_ts_1,
      files_ts_5,
      internals_ts_2,
      text_encoding_ts_5,
      runtime_ts_2,
      resources_ts_5,
      util_ts_5,
      RED_FAILED,
      GREEN_OK,
      YELLOW_IGNORED,
      disabledConsole,
      TEST_REGISTRY,
      encoder,
      TestApi;
    const __moduleName = context_60 && context_60.id;
    function delay(n) {
      return new Promise((resolve, _) => {
        setTimeout(resolve, n);
      });
    }
    function formatDuration(time = 0) {
      const timeStr = `(${time}ms)`;
      return colors_ts_1.gray(colors_ts_1.italic(timeStr));
    }
    // Wrap test function in additional assertion that makes sure
    // the test case does not leak async "ops" - ie. number of async
    // completed ops after the test is the same as number of dispatched
    // ops. Note that "unref" ops are ignored since in nature that are
    // optional.
    function assertOps(fn) {
      return async function asyncOpSanitizer() {
        const pre = runtime_ts_2.metrics();
        await fn();
        // Defer until next event loop turn - that way timeouts and intervals
        // cleared can actually be removed from resource table, otherwise
        // false positives may occur (https://github.com/denoland/deno/issues/4591)
        await delay(0);
        const post = runtime_ts_2.metrics();
        // We're checking diff because one might spawn HTTP server in the background
        // that will be a pending async op before test starts.
        const dispatchedDiff = post.opsDispatchedAsync - pre.opsDispatchedAsync;
        const completedDiff = post.opsCompletedAsync - pre.opsCompletedAsync;
        util_ts_5.assert(
          dispatchedDiff === completedDiff,
          `Test case is leaking async ops.
Before:
  - dispatched: ${pre.opsDispatchedAsync}
  - completed: ${pre.opsCompletedAsync}
After:
  - dispatched: ${post.opsDispatchedAsync}
  - completed: ${post.opsCompletedAsync}`
        );
      };
    }
    // Wrap test function in additional assertion that makes sure
    // the test case does not "leak" resources - ie. resource table after
    // the test has exactly the same contents as before the test.
    function assertResources(fn) {
      return async function resourceSanitizer() {
        const pre = resources_ts_5.resources();
        await fn();
        const post = resources_ts_5.resources();
        const preStr = JSON.stringify(pre, null, 2);
        const postStr = JSON.stringify(post, null, 2);
        const msg = `Test case is leaking resources.
Before: ${preStr}
After: ${postStr}`;
        util_ts_5.assert(preStr === postStr, msg);
      };
    }
    // Main test function provided by Deno, as you can see it merely
    // creates a new object with "name" and "fn" fields.
    function test(t, fn) {
      let testDef;
      if (typeof t === "string") {
        if (!fn || typeof fn != "function") {
          throw new TypeError("Missing test function");
        }
        if (!t) {
          throw new TypeError("The test name can't be empty");
        }
        testDef = { fn: fn, name: t, ignore: false };
      } else if (typeof t === "function") {
        if (!t.name) {
          throw new TypeError("The test function can't be anonymous");
        }
        testDef = { fn: t, name: t.name, ignore: false };
      } else {
        if (!t.fn) {
          throw new TypeError("Missing test function");
        }
        if (!t.name) {
          throw new TypeError("The test name can't be empty");
        }
        testDef = { ...t, ignore: Boolean(t.ignore) };
      }
      if (testDef.disableOpSanitizer !== true) {
        testDef.fn = assertOps(testDef.fn);
      }
      if (testDef.disableResourceSanitizer !== true) {
        testDef.fn = assertResources(testDef.fn);
      }
      TEST_REGISTRY.push(testDef);
    }
    exports_60("test", test);
    function log(msg, noNewLine = false) {
      if (!noNewLine) {
        msg += "\n";
      }
      // Using `stdout` here because it doesn't force new lines
      // compared to `console.log`; `core.print` on the other hand
      // is line-buffered and doesn't output message without newline
      files_ts_5.stdout.writeSync(encoder.encode(msg));
    }
    function reportToConsole(message) {
      if (message.start != null) {
        log(`running ${message.start.tests.length} tests`);
      } else if (message.testStart != null) {
        const { name } = message.testStart;
        log(`test ${name} ... `, true);
        return;
      } else if (message.testEnd != null) {
        switch (message.testEnd.status) {
          case "passed":
            log(`${GREEN_OK} ${formatDuration(message.testEnd.duration)}`);
            break;
          case "failed":
            log(`${RED_FAILED} ${formatDuration(message.testEnd.duration)}`);
            break;
          case "ignored":
            log(
              `${YELLOW_IGNORED} ${formatDuration(message.testEnd.duration)}`
            );
            break;
        }
      } else if (message.end != null) {
        const failures = message.end.results.filter((m) => m.error != null);
        if (failures.length > 0) {
          log(`\nfailures:\n`);
          for (const { name, error } of failures) {
            log(name);
            log(console_ts_1.stringifyArgs([error]));
            log("");
          }
          log(`failures:\n`);
          for (const { name } of failures) {
            log(`\t${name}`);
          }
        }
        log(
          `\ntest result: ${message.end.failed ? RED_FAILED : GREEN_OK}. ` +
            `${message.end.passed} passed; ${message.end.failed} failed; ` +
            `${message.end.ignored} ignored; ${message.end.measured} measured; ` +
            `${message.end.filtered} filtered out ` +
            `${formatDuration(message.end.duration)}\n`
        );
      }
    }
    function createFilterFn(filter, skip) {
      return (def) => {
        let passes = true;
        if (filter) {
          if (filter instanceof RegExp) {
            passes = passes && filter.test(def.name);
          } else {
            passes = passes && def.name.includes(filter);
          }
        }
        if (skip) {
          if (skip instanceof RegExp) {
            passes = passes && !skip.test(def.name);
          } else {
            passes = passes && !def.name.includes(skip);
          }
        }
        return passes;
      };
    }
    async function runTests({
      exitOnFail = true,
      failFast = false,
      filter = undefined,
      skip = undefined,
      disableLog = false,
      reportToConsole: reportToConsole_ = true,
      onMessage = undefined,
    } = {}) {
      const filterFn = createFilterFn(filter, skip);
      const testApi = new TestApi(TEST_REGISTRY, filterFn, failFast);
      // @ts-ignore
      const originalConsole = globalThis.console;
      if (disableLog) {
        // @ts-ignore
        globalThis.console = disabledConsole;
      }
      let endMsg;
      for await (const message of testApi) {
        if (onMessage != null) {
          await onMessage(message);
        }
        if (reportToConsole_) {
          reportToConsole(message);
        }
        if (message.end != null) {
          endMsg = message.end;
        }
      }
      if (disableLog) {
        // @ts-ignore
        globalThis.console = originalConsole;
      }
      if (endMsg.failed > 0 && exitOnFail) {
        os_ts_1.exit(1);
      }
      return endMsg;
    }
    exports_60("runTests", runTests);
    return {
      setters: [
        function (colors_ts_1_1) {
          colors_ts_1 = colors_ts_1_1;
        },
        function (os_ts_1_1) {
          os_ts_1 = os_ts_1_1;
        },
        function (console_ts_1_1) {
          console_ts_1 = console_ts_1_1;
        },
        function (files_ts_5_1) {
          files_ts_5 = files_ts_5_1;
        },
        function (internals_ts_2_1) {
          internals_ts_2 = internals_ts_2_1;
        },
        function (text_encoding_ts_5_1) {
          text_encoding_ts_5 = text_encoding_ts_5_1;
        },
        function (runtime_ts_2_1) {
          runtime_ts_2 = runtime_ts_2_1;
        },
        function (resources_ts_5_1) {
          resources_ts_5 = resources_ts_5_1;
        },
        function (util_ts_5_1) {
          util_ts_5 = util_ts_5_1;
        },
      ],
      execute: function () {
        RED_FAILED = colors_ts_1.red("FAILED");
        GREEN_OK = colors_ts_1.green("ok");
        YELLOW_IGNORED = colors_ts_1.yellow("ignored");
        disabledConsole = new console_ts_1.Console(() => {});
        TEST_REGISTRY = [];
        encoder = new text_encoding_ts_5.TextEncoder();
        internals_ts_2.exposeForTest("reportToConsole", reportToConsole);
        // TODO: already implements AsyncGenerator<RunTestsMessage>, but add as "implements to class"
        // TODO: implements PromiseLike<RunTestsEndResult>
        TestApi = class TestApi {
          constructor(tests, filterFn, failFast) {
            this.tests = tests;
            this.filterFn = filterFn;
            this.failFast = failFast;
            this.stats = {
              filtered: 0,
              ignored: 0,
              measured: 0,
              passed: 0,
              failed: 0,
            };
            this.testsToRun = tests.filter(filterFn);
            this.stats.filtered = tests.length - this.testsToRun.length;
          }
          async *[Symbol.asyncIterator]() {
            yield { start: { tests: this.testsToRun } };
            const results = [];
            const suiteStart = +new Date();
            for (const test of this.testsToRun) {
              const endMessage = {
                name: test.name,
                duration: 0,
              };
              yield { testStart: { ...test } };
              if (test.ignore) {
                endMessage.status = "ignored";
                this.stats.ignored++;
              } else {
                const start = +new Date();
                try {
                  await test.fn();
                  endMessage.status = "passed";
                  this.stats.passed++;
                } catch (err) {
                  endMessage.status = "failed";
                  endMessage.error = err;
                  this.stats.failed++;
                }
                endMessage.duration = +new Date() - start;
              }
              results.push(endMessage);
              yield { testEnd: endMessage };
              if (this.failFast && endMessage.error != null) {
                break;
              }
            }
            const duration = +new Date() - suiteStart;
            yield { end: { ...this.stats, duration, results } };
          }
        };
      },
    };
  }
);
System.register(
  "$deno$/symbols.ts",
  ["$deno$/internals.ts", "$deno$/web/console.ts"],
  function (exports_61, context_61) {
    "use strict";
    let internals_ts_3, console_ts_2;
    const __moduleName = context_61 && context_61.id;
    return {
      setters: [
        function (internals_ts_3_1) {
          internals_ts_3 = internals_ts_3_1;
        },
        function (console_ts_2_1) {
          console_ts_2 = console_ts_2_1;
        },
      ],
      execute: function () {
        exports_61("symbols", {
          internal: internals_ts_3.internalSymbol,
          customInspect: console_ts_2.customInspect,
        });
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/deno.ts",
  [
    "$deno$/buffer.ts",
    "$deno$/build.ts",
    "$deno$/ops/fs/chmod.ts",
    "$deno$/ops/fs/chown.ts",
    "$deno$/compiler/api.ts",
    "$deno$/web/console.ts",
    "$deno$/ops/fs/copy_file.ts",
    "$deno$/diagnostics.ts",
    "$deno$/ops/fs/dir.ts",
    "$deno$/ops/errors.ts",
    "$deno$/errors.ts",
    "$deno$/files.ts",
    "$deno$/ops/io.ts",
    "$deno$/ops/fs_events.ts",
    "$deno$/io.ts",
    "$deno$/ops/fs/link.ts",
    "$deno$/ops/fs/make_temp.ts",
    "$deno$/ops/runtime.ts",
    "$deno$/ops/fs/mkdir.ts",
    "$deno$/net.ts",
    "$deno$/ops/os.ts",
    "$deno$/permissions.ts",
    "$deno$/plugins.ts",
    "$deno$/ops/process.ts",
    "$deno$/process.ts",
    "$deno$/ops/fs/read_dir.ts",
    "$deno$/read_file.ts",
    "$deno$/ops/fs/read_link.ts",
    "$deno$/ops/fs/realpath.ts",
    "$deno$/ops/fs/remove.ts",
    "$deno$/ops/fs/rename.ts",
    "$deno$/ops/resources.ts",
    "$deno$/signals.ts",
    "$deno$/ops/fs/stat.ts",
    "$deno$/ops/fs/symlink.ts",
    "$deno$/tls.ts",
    "$deno$/ops/fs/truncate.ts",
    "$deno$/ops/tty.ts",
    "$deno$/ops/fs/umask.ts",
    "$deno$/ops/fs/utime.ts",
    "$deno$/version.ts",
    "$deno$/write_file.ts",
    "$deno$/testing.ts",
    "$deno$/core.ts",
    "$deno$/symbols.ts",
  ],
  function (exports_62, context_62) {
    "use strict";
    const __moduleName = context_62 && context_62.id;
    return {
      setters: [
        function (buffer_ts_4_1) {
          exports_62({
            Buffer: buffer_ts_4_1["Buffer"],
            readAll: buffer_ts_4_1["readAll"],
            readAllSync: buffer_ts_4_1["readAllSync"],
            writeAll: buffer_ts_4_1["writeAll"],
            writeAllSync: buffer_ts_4_1["writeAllSync"],
          });
        },
        function (build_ts_6_1) {
          exports_62({
            build: build_ts_6_1["build"],
          });
        },
        function (chmod_ts_2_1) {
          exports_62({
            chmodSync: chmod_ts_2_1["chmodSync"],
            chmod: chmod_ts_2_1["chmod"],
          });
        },
        function (chown_ts_1_1) {
          exports_62({
            chownSync: chown_ts_1_1["chownSync"],
            chown: chown_ts_1_1["chown"],
          });
        },
        function (api_ts_1_1) {
          exports_62({
            transpileOnly: api_ts_1_1["transpileOnly"],
            compile: api_ts_1_1["compile"],
            bundle: api_ts_1_1["bundle"],
          });
        },
        function (console_ts_3_1) {
          exports_62({
            inspect: console_ts_3_1["inspect"],
          });
        },
        function (copy_file_ts_1_1) {
          exports_62({
            copyFileSync: copy_file_ts_1_1["copyFileSync"],
            copyFile: copy_file_ts_1_1["copyFile"],
          });
        },
        function (diagnostics_ts_1_1) {
          exports_62({
            DiagnosticCategory: diagnostics_ts_1_1["DiagnosticCategory"],
          });
        },
        function (dir_ts_1_1) {
          exports_62({
            chdir: dir_ts_1_1["chdir"],
            cwd: dir_ts_1_1["cwd"],
          });
        },
        function (errors_ts_4_1) {
          exports_62({
            applySourceMap: errors_ts_4_1["applySourceMap"],
            formatDiagnostics: errors_ts_4_1["formatDiagnostics"],
          });
        },
        function (errors_ts_5_1) {
          exports_62({
            errors: errors_ts_5_1["errors"],
          });
        },
        function (files_ts_6_1) {
          exports_62({
            File: files_ts_6_1["File"],
            open: files_ts_6_1["open"],
            openSync: files_ts_6_1["openSync"],
            create: files_ts_6_1["create"],
            createSync: files_ts_6_1["createSync"],
            stdin: files_ts_6_1["stdin"],
            stdout: files_ts_6_1["stdout"],
            stderr: files_ts_6_1["stderr"],
            seek: files_ts_6_1["seek"],
            seekSync: files_ts_6_1["seekSync"],
          });
        },
        function (io_ts_5_1) {
          exports_62({
            read: io_ts_5_1["read"],
            readSync: io_ts_5_1["readSync"],
            write: io_ts_5_1["write"],
            writeSync: io_ts_5_1["writeSync"],
          });
        },
        function (fs_events_ts_1_1) {
          exports_62({
            fsEvents: fs_events_ts_1_1["fsEvents"],
          });
        },
        function (io_ts_6_1) {
          exports_62({
            EOF: io_ts_6_1["EOF"],
            copy: io_ts_6_1["copy"],
            toAsyncIterator: io_ts_6_1["toAsyncIterator"],
            SeekMode: io_ts_6_1["SeekMode"],
          });
        },
        function (link_ts_1_1) {
          exports_62({
            linkSync: link_ts_1_1["linkSync"],
            link: link_ts_1_1["link"],
          });
        },
        function (make_temp_ts_1_1) {
          exports_62({
            makeTempDirSync: make_temp_ts_1_1["makeTempDirSync"],
            makeTempDir: make_temp_ts_1_1["makeTempDir"],
            makeTempFileSync: make_temp_ts_1_1["makeTempFileSync"],
            makeTempFile: make_temp_ts_1_1["makeTempFile"],
          });
        },
        function (runtime_ts_3_1) {
          exports_62({
            metrics: runtime_ts_3_1["metrics"],
          });
        },
        function (mkdir_ts_1_1) {
          exports_62({
            mkdirSync: mkdir_ts_1_1["mkdirSync"],
            mkdir: mkdir_ts_1_1["mkdir"],
          });
        },
        function (net_ts_2_1) {
          exports_62({
            connect: net_ts_2_1["connect"],
            listen: net_ts_2_1["listen"],
            ShutdownMode: net_ts_2_1["ShutdownMode"],
            shutdown: net_ts_2_1["shutdown"],
          });
        },
        function (os_ts_2_1) {
          exports_62({
            dir: os_ts_2_1["dir"],
            env: os_ts_2_1["env"],
            exit: os_ts_2_1["exit"],
            execPath: os_ts_2_1["execPath"],
            hostname: os_ts_2_1["hostname"],
            loadavg: os_ts_2_1["loadavg"],
            osRelease: os_ts_2_1["osRelease"],
          });
        },
        function (permissions_ts_1_1) {
          exports_62({
            permissions: permissions_ts_1_1["permissions"],
            PermissionStatus: permissions_ts_1_1["PermissionStatus"],
            Permissions: permissions_ts_1_1["Permissions"],
          });
        },
        function (plugins_ts_2_1) {
          exports_62({
            openPlugin: plugins_ts_2_1["openPlugin"],
          });
        },
        function (process_ts_2_1) {
          exports_62({
            kill: process_ts_2_1["kill"],
          });
        },
        function (process_ts_3_1) {
          exports_62({
            run: process_ts_3_1["run"],
            Process: process_ts_3_1["Process"],
          });
        },
        function (read_dir_ts_1_1) {
          exports_62({
            readdirSync: read_dir_ts_1_1["readdirSync"],
            readdir: read_dir_ts_1_1["readdir"],
          });
        },
        function (read_file_ts_1_1) {
          exports_62({
            readFileSync: read_file_ts_1_1["readFileSync"],
            readFile: read_file_ts_1_1["readFile"],
          });
        },
        function (read_link_ts_1_1) {
          exports_62({
            readlinkSync: read_link_ts_1_1["readlinkSync"],
            readlink: read_link_ts_1_1["readlink"],
          });
        },
        function (realpath_ts_1_1) {
          exports_62({
            realpathSync: realpath_ts_1_1["realpathSync"],
            realpath: realpath_ts_1_1["realpath"],
          });
        },
        function (remove_ts_1_1) {
          exports_62({
            removeSync: remove_ts_1_1["removeSync"],
            remove: remove_ts_1_1["remove"],
          });
        },
        function (rename_ts_1_1) {
          exports_62({
            renameSync: rename_ts_1_1["renameSync"],
            rename: rename_ts_1_1["rename"],
          });
        },
        function (resources_ts_6_1) {
          exports_62({
            resources: resources_ts_6_1["resources"],
            close: resources_ts_6_1["close"],
          });
        },
        function (signals_ts_1_1) {
          exports_62({
            signal: signals_ts_1_1["signal"],
            signals: signals_ts_1_1["signals"],
            Signal: signals_ts_1_1["Signal"],
            SignalStream: signals_ts_1_1["SignalStream"],
          });
        },
        function (stat_ts_2_1) {
          exports_62({
            statSync: stat_ts_2_1["statSync"],
            lstatSync: stat_ts_2_1["lstatSync"],
            stat: stat_ts_2_1["stat"],
            lstat: stat_ts_2_1["lstat"],
          });
        },
        function (symlink_ts_1_1) {
          exports_62({
            symlinkSync: symlink_ts_1_1["symlinkSync"],
            symlink: symlink_ts_1_1["symlink"],
          });
        },
        function (tls_ts_1_1) {
          exports_62({
            connectTLS: tls_ts_1_1["connectTLS"],
            listenTLS: tls_ts_1_1["listenTLS"],
          });
        },
        function (truncate_ts_1_1) {
          exports_62({
            truncateSync: truncate_ts_1_1["truncateSync"],
            truncate: truncate_ts_1_1["truncate"],
          });
        },
        function (tty_ts_1_1) {
          exports_62({
            isatty: tty_ts_1_1["isatty"],
            setRaw: tty_ts_1_1["setRaw"],
          });
        },
        function (umask_ts_1_1) {
          exports_62({
            umask: umask_ts_1_1["umask"],
          });
        },
        function (utime_ts_1_1) {
          exports_62({
            utimeSync: utime_ts_1_1["utimeSync"],
            utime: utime_ts_1_1["utime"],
          });
        },
        function (version_ts_1_1) {
          exports_62({
            version: version_ts_1_1["version"],
          });
        },
        function (write_file_ts_1_1) {
          exports_62({
            writeFileSync: write_file_ts_1_1["writeFileSync"],
            writeFile: write_file_ts_1_1["writeFile"],
          });
        },
        function (testing_ts_1_1) {
          exports_62({
            runTests: testing_ts_1_1["runTests"],
            test: testing_ts_1_1["test"],
          });
        },
        function (core_ts_4_1) {
          exports_62({
            core: core_ts_4_1["core"],
          });
        },
        function (symbols_ts_1_1) {
          exports_62({
            symbols: symbols_ts_1_1["symbols"],
          });
        },
      ],
      execute: function () {
        exports_62("args", []);
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/colors.ts", ["$deno$/deno.ts"], function (
  exports_63,
  context_63
) {
  "use strict";
  let deno_ts_1, enabled, ANSI_PATTERN;
  const __moduleName = context_63 && context_63.id;
  function code(open, close) {
    return {
      open: `\x1b[${open}m`,
      close: `\x1b[${close}m`,
      regexp: new RegExp(`\\x1b\\[${close}m`, "g"),
    };
  }
  function run(str, code) {
    return enabled
      ? `${code.open}${str.replace(code.regexp, code.open)}${code.close}`
      : str;
  }
  function bold(str) {
    return run(str, code(1, 22));
  }
  exports_63("bold", bold);
  function italic(str) {
    return run(str, code(3, 23));
  }
  exports_63("italic", italic);
  function yellow(str) {
    return run(str, code(33, 39));
  }
  exports_63("yellow", yellow);
  function cyan(str) {
    return run(str, code(36, 39));
  }
  exports_63("cyan", cyan);
  function red(str) {
    return run(str, code(31, 39));
  }
  exports_63("red", red);
  function green(str) {
    return run(str, code(32, 39));
  }
  exports_63("green", green);
  function bgRed(str) {
    return run(str, code(41, 49));
  }
  exports_63("bgRed", bgRed);
  function white(str) {
    return run(str, code(37, 39));
  }
  exports_63("white", white);
  function gray(str) {
    return run(str, code(90, 39));
  }
  exports_63("gray", gray);
  function stripColor(string) {
    return string.replace(ANSI_PATTERN, "");
  }
  exports_63("stripColor", stripColor);
  return {
    setters: [
      function (deno_ts_1_1) {
        deno_ts_1 = deno_ts_1_1;
      },
    ],
    execute: function () {
      enabled = !deno_ts_1.noColor;
      // https://github.com/chalk/ansi-regex/blob/2b56fb0c7a07108e5b54241e8faec160d393aedb/index.js
      ANSI_PATTERN = new RegExp(
        [
          "[\\u001B\\u009B][[\\]()#;?]*(?:(?:(?:[a-zA-Z\\d]*(?:;[-a-zA-Z\\d\\/#&.:=?%@~_]*)*)?\\u0007)",
          "(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PR-TZcf-ntqry=><~]))",
        ].join("|"),
        "g"
      );
    },
  };
});
System.register(
  "$deno$/error_stack.ts",
  [
    "$deno$/colors.ts",
    "$deno$/ops/errors.ts",
    "$deno$/util.ts",
    "$deno$/internals.ts",
  ],
  function (exports_64, context_64) {
    "use strict";
    let colors, errors_ts_6, util_ts_6, internals_ts_4;
    const __moduleName = context_64 && context_64.id;
    function patchCallSite(callSite, location) {
      return {
        getThis() {
          return callSite.getThis();
        },
        getTypeName() {
          return callSite.getTypeName();
        },
        getFunction() {
          return callSite.getFunction();
        },
        getFunctionName() {
          return callSite.getFunctionName();
        },
        getMethodName() {
          return callSite.getMethodName();
        },
        getFileName() {
          return location.fileName;
        },
        getLineNumber() {
          return location.lineNumber;
        },
        getColumnNumber() {
          return location.columnNumber;
        },
        getEvalOrigin() {
          return callSite.getEvalOrigin();
        },
        isToplevel() {
          return callSite.isToplevel();
        },
        isEval() {
          return callSite.isEval();
        },
        isNative() {
          return callSite.isNative();
        },
        isConstructor() {
          return callSite.isConstructor();
        },
        isAsync() {
          return callSite.isAsync();
        },
        isPromiseAll() {
          return callSite.isPromiseAll();
        },
        getPromiseIndex() {
          return callSite.getPromiseIndex();
        },
      };
    }
    function getMethodCall(callSite) {
      let result = "";
      const typeName = callSite.getTypeName();
      const methodName = callSite.getMethodName();
      const functionName = callSite.getFunctionName();
      if (functionName) {
        if (typeName) {
          const startsWithTypeName = functionName.startsWith(typeName);
          if (!startsWithTypeName) {
            result += `${typeName}.`;
          }
        }
        result += functionName;
        if (methodName) {
          if (!functionName.endsWith(methodName)) {
            result += ` [as ${methodName}]`;
          }
        }
      } else {
        if (typeName) {
          result += `${typeName}.`;
        }
        if (methodName) {
          result += methodName;
        } else {
          result += "<anonymous>";
        }
      }
      return result;
    }
    function getFileLocation(callSite, isInternal = false) {
      const cyan = isInternal ? colors.gray : colors.cyan;
      const yellow = isInternal ? colors.gray : colors.yellow;
      const black = isInternal ? colors.gray : (s) => s;
      if (callSite.isNative()) {
        return cyan("native");
      }
      let result = "";
      const fileName = callSite.getFileName();
      if (!fileName && callSite.isEval()) {
        const evalOrigin = callSite.getEvalOrigin();
        util_ts_6.assert(evalOrigin != null);
        result += cyan(`${evalOrigin}, `);
      }
      if (fileName) {
        result += cyan(fileName);
      } else {
        result += cyan("<anonymous>");
      }
      const lineNumber = callSite.getLineNumber();
      if (lineNumber != null) {
        result += `${black(":")}${yellow(lineNumber.toString())}`;
        const columnNumber = callSite.getColumnNumber();
        if (columnNumber != null) {
          result += `${black(":")}${yellow(columnNumber.toString())}`;
        }
      }
      return result;
    }
    function callSiteToString(callSite, isInternal = false) {
      const cyan = isInternal ? colors.gray : colors.cyan;
      const black = isInternal ? colors.gray : (s) => s;
      let result = "";
      const functionName = callSite.getFunctionName();
      const isTopLevel = callSite.isToplevel();
      const isAsync = callSite.isAsync();
      const isPromiseAll = callSite.isPromiseAll();
      const isConstructor = callSite.isConstructor();
      const isMethodCall = !(isTopLevel || isConstructor);
      if (isAsync) {
        result += colors.gray("async ");
      }
      if (isPromiseAll) {
        result += colors.bold(
          colors.italic(
            black(`Promise.all (index ${callSite.getPromiseIndex()})`)
          )
        );
        return result;
      }
      if (isMethodCall) {
        result += colors.bold(colors.italic(black(getMethodCall(callSite))));
      } else if (isConstructor) {
        result += colors.gray("new ");
        if (functionName) {
          result += colors.bold(colors.italic(black(functionName)));
        } else {
          result += cyan("<anonymous>");
        }
      } else if (functionName) {
        result += colors.bold(colors.italic(black(functionName)));
      } else {
        result += getFileLocation(callSite, isInternal);
        return result;
      }
      result += ` ${black("(")}${getFileLocation(callSite, isInternal)}${black(
        ")"
      )}`;
      return result;
    }
    function evaluateCallSite(callSite) {
      return {
        this: callSite.getThis(),
        typeName: callSite.getTypeName(),
        function: callSite.getFunction(),
        functionName: callSite.getFunctionName(),
        methodName: callSite.getMethodName(),
        fileName: callSite.getFileName(),
        lineNumber: callSite.getLineNumber(),
        columnNumber: callSite.getColumnNumber(),
        evalOrigin: callSite.getEvalOrigin(),
        isToplevel: callSite.isToplevel(),
        isEval: callSite.isEval(),
        isNative: callSite.isNative(),
        isConstructor: callSite.isConstructor(),
        isAsync: callSite.isAsync(),
        isPromiseAll: callSite.isPromiseAll(),
        promiseIndex: callSite.getPromiseIndex(),
      };
    }
    function prepareStackTrace(error, structuredStackTrace) {
      Object.defineProperties(error, {
        __callSiteEvals: { value: [] },
        __formattedFrames: { value: [] },
      });
      const errorString =
        `${error.name}: ${error.message}\n` +
        structuredStackTrace
          .map((callSite) => {
            const fileName = callSite.getFileName();
            const lineNumber = callSite.getLineNumber();
            const columnNumber = callSite.getColumnNumber();
            if (fileName && lineNumber != null && columnNumber != null) {
              return patchCallSite(
                callSite,
                errors_ts_6.applySourceMap({
                  fileName,
                  lineNumber,
                  columnNumber,
                })
              );
            }
            return callSite;
          })
          .map((callSite) => {
            // @ts-ignore
            error.__callSiteEvals.push(
              Object.freeze(evaluateCallSite(callSite))
            );
            const isInternal =
              callSite.getFileName()?.startsWith("$deno$") ?? false;
            const string = callSiteToString(callSite, isInternal);
            // @ts-ignore
            error.__formattedFrames.push(string);
            return `    at ${colors.stripColor(string)}`;
          })
          .join("\n");
      // @ts-ignore
      Object.freeze(error.__callSiteEvals);
      // @ts-ignore
      Object.freeze(error.__formattedFrames);
      return errorString;
    }
    // @internal
    function setPrepareStackTrace(ErrorConstructor) {
      ErrorConstructor.prepareStackTrace = prepareStackTrace;
    }
    exports_64("setPrepareStackTrace", setPrepareStackTrace);
    return {
      setters: [
        function (colors_1) {
          colors = colors_1;
        },
        function (errors_ts_6_1) {
          errors_ts_6 = errors_ts_6_1;
        },
        function (util_ts_6_1) {
          util_ts_6 = util_ts_6_1;
        },
        function (internals_ts_4_1) {
          internals_ts_4 = internals_ts_4_1;
        },
      ],
      execute: function () {
        internals_ts_4.exposeForTest(
          "setPrepareStackTrace",
          setPrepareStackTrace
        );
      },
    };
  }
);
System.register(
  "$deno$/ops/timers.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_65, context_65) {
    "use strict";
    let dispatch_json_ts_32;
    const __moduleName = context_65 && context_65.id;
    function stopGlobalTimer() {
      dispatch_json_ts_32.sendSync("op_global_timer_stop");
    }
    exports_65("stopGlobalTimer", stopGlobalTimer);
    async function startGlobalTimer(timeout) {
      await dispatch_json_ts_32.sendAsync("op_global_timer", { timeout });
    }
    exports_65("startGlobalTimer", startGlobalTimer);
    function now() {
      return dispatch_json_ts_32.sendSync("op_now");
    }
    exports_65("now", now);
    return {
      setters: [
        function (dispatch_json_ts_32_1) {
          dispatch_json_ts_32 = dispatch_json_ts_32_1;
        },
      ],
      execute: function () {},
    };
  }
);
// Derived from https://github.com/vadimg/js_bintrees. MIT Licensed.
System.register("$deno$/rbtree.ts", ["$deno$/util.ts"], function (
  exports_66,
  context_66
) {
  "use strict";
  let util_ts_7, RBNode, RBTree;
  const __moduleName = context_66 && context_66.id;
  function isRed(node) {
    return node !== null && node.red;
  }
  function singleRotate(root, dir) {
    const save = root.getChild(!dir);
    util_ts_7.assert(save);
    root.setChild(!dir, save.getChild(dir));
    save.setChild(dir, root);
    root.red = true;
    save.red = false;
    return save;
  }
  function doubleRotate(root, dir) {
    root.setChild(!dir, singleRotate(root.getChild(!dir), !dir));
    return singleRotate(root, dir);
  }
  return {
    setters: [
      function (util_ts_7_1) {
        util_ts_7 = util_ts_7_1;
      },
    ],
    execute: function () {
      RBNode = class RBNode {
        constructor(data) {
          this.data = data;
          this.left = null;
          this.right = null;
          this.red = true;
        }
        getChild(dir) {
          return dir ? this.right : this.left;
        }
        setChild(dir, val) {
          if (dir) {
            this.right = val;
          } else {
            this.left = val;
          }
        }
      };
      RBTree = class RBTree {
        constructor(comparator) {
          this.#comparator = comparator;
          this.#root = null;
        }
        #comparator;
        #root;
        /** Returns `null` if tree is empty. */
        min() {
          let res = this.#root;
          if (res === null) {
            return null;
          }
          while (res.left !== null) {
            res = res.left;
          }
          return res.data;
        }
        /** Returns node `data` if found, `null` otherwise. */
        find(data) {
          let res = this.#root;
          while (res !== null) {
            const c = this.#comparator(data, res.data);
            if (c === 0) {
              return res.data;
            } else {
              res = res.getChild(c > 0);
            }
          }
          return null;
        }
        /** returns `true` if inserted, `false` if duplicate. */
        insert(data) {
          let ret = false;
          if (this.#root === null) {
            // empty tree
            this.#root = new RBNode(data);
            ret = true;
          } else {
            const head = new RBNode(null); // fake tree root
            let dir = 0;
            let last = 0;
            // setup
            let gp = null; // grandparent
            let ggp = head; // grand-grand-parent
            let p = null; // parent
            let node = this.#root;
            ggp.right = this.#root;
            // search down
            while (true) {
              if (node === null) {
                // insert new node at the bottom
                node = new RBNode(data);
                p.setChild(dir, node);
                ret = true;
              } else if (isRed(node.left) && isRed(node.right)) {
                // color flip
                node.red = true;
                node.left.red = false;
                node.right.red = false;
              }
              // fix red violation
              if (isRed(node) && isRed(p)) {
                const dir2 = ggp.right === gp;
                util_ts_7.assert(gp);
                if (node === p.getChild(last)) {
                  ggp.setChild(dir2, singleRotate(gp, !last));
                } else {
                  ggp.setChild(dir2, doubleRotate(gp, !last));
                }
              }
              const cmp = this.#comparator(node.data, data);
              // stop if found
              if (cmp === 0) {
                break;
              }
              last = dir;
              dir = Number(cmp < 0); // Fix type
              // update helpers
              if (gp !== null) {
                ggp = gp;
              }
              gp = p;
              p = node;
              node = node.getChild(dir);
            }
            // update root
            this.#root = head.right;
          }
          // make root black
          this.#root.red = false;
          return ret;
        }
        /** Returns `true` if removed, `false` if not found. */
        remove(data) {
          if (this.#root === null) {
            return false;
          }
          const head = new RBNode(null); // fake tree root
          let node = head;
          node.right = this.#root;
          let p = null; // parent
          let gp = null; // grand parent
          let found = null; // found item
          let dir = 1;
          while (node.getChild(dir) !== null) {
            const last = dir;
            // update helpers
            gp = p;
            p = node;
            node = node.getChild(dir);
            const cmp = this.#comparator(data, node.data);
            dir = cmp > 0;
            // save found node
            if (cmp === 0) {
              found = node;
            }
            // push the red node down
            if (!isRed(node) && !isRed(node.getChild(dir))) {
              if (isRed(node.getChild(!dir))) {
                const sr = singleRotate(node, dir);
                p.setChild(last, sr);
                p = sr;
              } else if (!isRed(node.getChild(!dir))) {
                const sibling = p.getChild(!last);
                if (sibling !== null) {
                  if (
                    !isRed(sibling.getChild(!last)) &&
                    !isRed(sibling.getChild(last))
                  ) {
                    // color flip
                    p.red = false;
                    sibling.red = true;
                    node.red = true;
                  } else {
                    util_ts_7.assert(gp);
                    const dir2 = gp.right === p;
                    if (isRed(sibling.getChild(last))) {
                      gp.setChild(dir2, doubleRotate(p, last));
                    } else if (isRed(sibling.getChild(!last))) {
                      gp.setChild(dir2, singleRotate(p, last));
                    }
                    // ensure correct coloring
                    const gpc = gp.getChild(dir2);
                    util_ts_7.assert(gpc);
                    gpc.red = true;
                    node.red = true;
                    util_ts_7.assert(gpc.left);
                    gpc.left.red = false;
                    util_ts_7.assert(gpc.right);
                    gpc.right.red = false;
                  }
                }
              }
            }
          }
          // replace and remove if found
          if (found !== null) {
            found.data = node.data;
            util_ts_7.assert(p);
            p.setChild(p.right === node, node.getChild(node.left === null));
          }
          // update root and make it black
          this.#root = head.right;
          if (this.#root !== null) {
            this.#root.red = false;
          }
          return found !== null;
        }
      };
      exports_66("RBTree", RBTree);
    },
  };
});
System.register(
  "$deno$/web/timers.ts",
  ["$deno$/util.ts", "$deno$/ops/timers.ts", "$deno$/rbtree.ts"],
  function (exports_67, context_67) {
    "use strict";
    let util_ts_8,
      timers_ts_1,
      rbtree_ts_1,
      console,
      TIMEOUT_MAX,
      globalTimeoutDue,
      nextTimerId,
      idMap,
      dueTree,
      pendingEvents,
      pendingFireTimers;
    const __moduleName = context_67 && context_67.id;
    function clearGlobalTimeout() {
      globalTimeoutDue = null;
      timers_ts_1.stopGlobalTimer();
    }
    /** Process and run a single ready timer macrotask.
     * This function should be registered through Deno.core.setMacrotaskCallback.
     * Returns true when all ready macrotasks have been processed, false if more
     * ready ones are available. The Isolate future would rely on the return value
     * to repeatedly invoke this function until depletion. Multiple invocations
     * of this function one at a time ensures newly ready microtasks are processed
     * before next macrotask timer callback is invoked. */
    function handleTimerMacrotask() {
      if (pendingFireTimers.length > 0) {
        fire(pendingFireTimers.shift());
        return pendingFireTimers.length === 0;
      }
      return true;
    }
    exports_67("handleTimerMacrotask", handleTimerMacrotask);
    async function setGlobalTimeout(due, now) {
      // Since JS and Rust don't use the same clock, pass the time to rust as a
      // relative time value. On the Rust side we'll turn that into an absolute
      // value again.
      const timeout = due - now;
      util_ts_8.assert(timeout >= 0);
      // Send message to the backend.
      globalTimeoutDue = due;
      pendingEvents++;
      // FIXME(bartlomieju): this is problematic, because `clearGlobalTimeout`
      // is synchronous. That means that timer is cancelled, but this promise is still pending
      // until next turn of event loop. This leads to "leaking of async ops" in tests;
      // because `clearTimeout/clearInterval` might be the last statement in test function
      // `opSanitizer` will immediately complain that there is pending op going on, unless
      // some timeout/defer is put in place to allow promise resolution.
      // Ideally `clearGlobalTimeout` doesn't return until this op is resolved, but
      // I'm not if that's possible.
      await timers_ts_1.startGlobalTimer(timeout);
      pendingEvents--;
      // eslint-disable-next-line @typescript-eslint/no-use-before-define
      prepareReadyTimers();
    }
    function prepareReadyTimers() {
      const now = Date.now();
      // Bail out if we're not expecting the global timer to fire.
      if (globalTimeoutDue === null || pendingEvents > 0) {
        return;
      }
      // After firing the timers that are due now, this will hold the first timer
      // list that hasn't fired yet.
      let nextDueNode;
      while ((nextDueNode = dueTree.min()) !== null && nextDueNode.due <= now) {
        dueTree.remove(nextDueNode);
        // Fire all the timers in the list.
        for (const timer of nextDueNode.timers) {
          // With the list dropped, the timer is no longer scheduled.
          timer.scheduled = false;
          // Place the callback to pending timers to fire.
          pendingFireTimers.push(timer);
        }
      }
      setOrClearGlobalTimeout(nextDueNode && nextDueNode.due, now);
    }
    function setOrClearGlobalTimeout(due, now) {
      if (due == null) {
        clearGlobalTimeout();
      } else {
        setGlobalTimeout(due, now);
      }
    }
    function schedule(timer, now) {
      util_ts_8.assert(!timer.scheduled);
      util_ts_8.assert(now <= timer.due);
      // Find or create the list of timers that will fire at point-in-time `due`.
      const maybeNewDueNode = { due: timer.due, timers: [] };
      let dueNode = dueTree.find(maybeNewDueNode);
      if (dueNode === null) {
        dueTree.insert(maybeNewDueNode);
        dueNode = maybeNewDueNode;
      }
      // Append the newly scheduled timer to the list and mark it as scheduled.
      dueNode.timers.push(timer);
      timer.scheduled = true;
      // If the new timer is scheduled to fire before any timer that existed before,
      // update the global timeout to reflect this.
      if (globalTimeoutDue === null || globalTimeoutDue > timer.due) {
        setOrClearGlobalTimeout(timer.due, now);
      }
    }
    function unschedule(timer) {
      // Check if our timer is pending scheduling or pending firing.
      // If either is true, they are not in tree, and their idMap entry
      // will be deleted soon. Remove it from queue.
      let index = -1;
      if ((index = pendingFireTimers.indexOf(timer)) >= 0) {
        pendingFireTimers.splice(index);
        return;
      }
      // If timer is not in the 2 pending queues and is unscheduled,
      // it is not in the tree.
      if (!timer.scheduled) {
        return;
      }
      const searchKey = { due: timer.due, timers: [] };
      // Find the list of timers that will fire at point-in-time `due`.
      const list = dueTree.find(searchKey).timers;
      if (list.length === 1) {
        // Time timer is the only one in the list. Remove the entire list.
        util_ts_8.assert(list[0] === timer);
        dueTree.remove(searchKey);
        // If the unscheduled timer was 'next up', find when the next timer that
        // still exists is due, and update the global alarm accordingly.
        if (timer.due === globalTimeoutDue) {
          const nextDueNode = dueTree.min();
          setOrClearGlobalTimeout(nextDueNode && nextDueNode.due, Date.now());
        }
      } else {
        // Multiple timers that are due at the same point in time.
        // Remove this timer from the list.
        const index = list.indexOf(timer);
        util_ts_8.assert(index > -1);
        list.splice(index, 1);
      }
    }
    function fire(timer) {
      // If the timer isn't found in the ID map, that means it has been cancelled
      // between the timer firing and the promise callback (this function).
      if (!idMap.has(timer.id)) {
        return;
      }
      // Reschedule the timer if it is a repeating one, otherwise drop it.
      if (!timer.repeat) {
        // One-shot timer: remove the timer from this id-to-timer map.
        idMap.delete(timer.id);
      } else {
        // Interval timer: compute when timer was supposed to fire next.
        // However make sure to never schedule the next interval in the past.
        const now = Date.now();
        timer.due = Math.max(now, timer.due + timer.delay);
        schedule(timer, now);
      }
      // Call the user callback. Intermediate assignment is to avoid leaking `this`
      // to it, while also keeping the stack trace neat when it shows up in there.
      const callback = timer.callback;
      callback();
    }
    function checkThis(thisArg) {
      if (thisArg !== null && thisArg !== undefined && thisArg !== globalThis) {
        throw new TypeError("Illegal invocation");
      }
    }
    function checkBigInt(n) {
      if (typeof n === "bigint") {
        throw new TypeError("Cannot convert a BigInt value to a number");
      }
    }
    function setTimer(cb, delay, args, repeat) {
      // Bind `args` to the callback and bind `this` to globalThis(global).
      const callback = cb.bind(globalThis, ...args);
      // In the browser, the delay value must be coercible to an integer between 0
      // and INT32_MAX. Any other value will cause the timer to fire immediately.
      // We emulate this behavior.
      const now = Date.now();
      if (delay > TIMEOUT_MAX) {
        console.warn(
          `${delay} does not fit into` +
            " a 32-bit signed integer." +
            "\nTimeout duration was set to 1."
        );
        delay = 1;
      }
      delay = Math.max(0, delay | 0);
      // Create a new, unscheduled timer object.
      const timer = {
        id: nextTimerId++,
        callback,
        args,
        delay,
        due: now + delay,
        repeat,
        scheduled: false,
      };
      // Register the timer's existence in the id-to-timer map.
      idMap.set(timer.id, timer);
      // Schedule the timer in the due table.
      schedule(timer, now);
      return timer.id;
    }
    function setTimeout(cb, delay = 0, ...args) {
      checkBigInt(delay);
      // @ts-ignore
      checkThis(this);
      return setTimer(cb, delay, args, false);
    }
    exports_67("setTimeout", setTimeout);
    function setInterval(cb, delay = 0, ...args) {
      checkBigInt(delay);
      // @ts-ignore
      checkThis(this);
      return setTimer(cb, delay, args, true);
    }
    exports_67("setInterval", setInterval);
    function clearTimer(id) {
      id = Number(id);
      const timer = idMap.get(id);
      if (timer === undefined) {
        // Timer doesn't exist any more or never existed. This is not an error.
        return;
      }
      // Unschedule the timer if it is currently scheduled, and forget about it.
      unschedule(timer);
      idMap.delete(timer.id);
    }
    function clearTimeout(id = 0) {
      checkBigInt(id);
      if (id === 0) {
        return;
      }
      clearTimer(id);
    }
    exports_67("clearTimeout", clearTimeout);
    function clearInterval(id = 0) {
      checkBigInt(id);
      if (id === 0) {
        return;
      }
      clearTimer(id);
    }
    exports_67("clearInterval", clearInterval);
    return {
      setters: [
        function (util_ts_8_1) {
          util_ts_8 = util_ts_8_1;
        },
        function (timers_ts_1_1) {
          timers_ts_1 = timers_ts_1_1;
        },
        function (rbtree_ts_1_1) {
          rbtree_ts_1 = rbtree_ts_1_1;
        },
      ],
      execute: function () {
        console = globalThis.console;
        // Timeout values > TIMEOUT_MAX are set to 1.
        TIMEOUT_MAX = 2 ** 31 - 1;
        globalTimeoutDue = null;
        nextTimerId = 1;
        idMap = new Map();
        dueTree = new rbtree_ts_1.RBTree((a, b) => a.due - b.due);
        pendingEvents = 0;
        pendingFireTimers = [];
      },
    };
  }
);
System.register(
  "$deno$/runtime.ts",
  [
    "$deno$/core.ts",
    "$deno$/ops/dispatch_minimal.ts",
    "$deno$/ops/dispatch_json.ts",
    "$deno$/util.ts",
    "$deno$/build.ts",
    "$deno$/version.ts",
    "$deno$/error_stack.ts",
    "$deno$/ops/runtime.ts",
    "$deno$/web/timers.ts",
  ],
  function (exports_68, context_68) {
    "use strict";
    let core_ts_5,
      dispatchMinimal,
      dispatchJson,
      util,
      build_ts_7,
      version_ts_2,
      error_stack_ts_1,
      runtime_ts_4,
      timers_ts_2,
      OPS_CACHE;
    const __moduleName = context_68 && context_68.id;
    function getAsyncHandler(opName) {
      switch (opName) {
        case "op_write":
        case "op_read":
          return dispatchMinimal.asyncMsgFromRust;
        default:
          return dispatchJson.asyncMsgFromRust;
      }
    }
    // TODO(bartlomieju): temporary solution, must be fixed when moving
    // dispatches to separate crates
    function initOps() {
      exports_68("OPS_CACHE", (OPS_CACHE = core_ts_5.core.ops()));
      for (const [name, opId] of Object.entries(OPS_CACHE)) {
        core_ts_5.core.setAsyncHandler(opId, getAsyncHandler(name));
      }
      core_ts_5.core.setMacrotaskCallback(timers_ts_2.handleTimerMacrotask);
    }
    exports_68("initOps", initOps);
    function start(source) {
      initOps();
      // First we send an empty `Start` message to let the privileged side know we
      // are ready. The response should be a `StartRes` message containing the CLI
      // args and other info.
      const s = runtime_ts_4.start();
      version_ts_2.setVersions(s.denoVersion, s.v8Version, s.tsVersion);
      build_ts_7.setBuildInfo(s.os, s.arch);
      util.setLogDebug(s.debugFlag, source);
      error_stack_ts_1.setPrepareStackTrace(Error);
      return s;
    }
    exports_68("start", start);
    return {
      setters: [
        function (core_ts_5_1) {
          core_ts_5 = core_ts_5_1;
        },
        function (dispatchMinimal_1) {
          dispatchMinimal = dispatchMinimal_1;
        },
        function (dispatchJson_1) {
          dispatchJson = dispatchJson_1;
        },
        function (util_3) {
          util = util_3;
        },
        function (build_ts_7_1) {
          build_ts_7 = build_ts_7_1;
        },
        function (version_ts_2_1) {
          version_ts_2 = version_ts_2_1;
        },
        function (error_stack_ts_1_1) {
          error_stack_ts_1 = error_stack_ts_1_1;
        },
        function (runtime_ts_4_1) {
          runtime_ts_4 = runtime_ts_4_1;
        },
        function (timers_ts_2_1) {
          timers_ts_2 = timers_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/ops/dispatch_json.ts",
  ["$deno$/util.ts", "$deno$/core.ts", "$deno$/runtime.ts", "$deno$/errors.ts"],
  function (exports_69, context_69) {
    "use strict";
    let util,
      core_ts_6,
      runtime_ts_5,
      errors_ts_7,
      promiseTable,
      _nextPromiseId;
    const __moduleName = context_69 && context_69.id;
    function nextPromiseId() {
      return _nextPromiseId++;
    }
    function decode(ui8) {
      const s = core_ts_6.core.decode(ui8);
      return JSON.parse(s);
    }
    function encode(args) {
      const s = JSON.stringify(args);
      return core_ts_6.core.encode(s);
    }
    function unwrapResponse(res) {
      if (res.err != null) {
        throw new (errors_ts_7.getErrorClass(res.err.kind))(res.err.message);
      }
      util.assert(res.ok != null);
      return res.ok;
    }
    function asyncMsgFromRust(resUi8) {
      const res = decode(resUi8);
      util.assert(res.promiseId != null);
      const promise = promiseTable[res.promiseId];
      util.assert(promise != null);
      delete promiseTable[res.promiseId];
      promise.resolve(res);
    }
    exports_69("asyncMsgFromRust", asyncMsgFromRust);
    function sendSync(opName, args = {}, zeroCopy) {
      const opId = runtime_ts_5.OPS_CACHE[opName];
      util.log("sendSync", opName, opId);
      const argsUi8 = encode(args);
      const resUi8 = core_ts_6.core.dispatch(opId, argsUi8, zeroCopy);
      util.assert(resUi8 != null);
      const res = decode(resUi8);
      util.assert(res.promiseId == null);
      return unwrapResponse(res);
    }
    exports_69("sendSync", sendSync);
    async function sendAsync(opName, args = {}, zeroCopy) {
      const opId = runtime_ts_5.OPS_CACHE[opName];
      util.log("sendAsync", opName, opId);
      const promiseId = nextPromiseId();
      args = Object.assign(args, { promiseId });
      const promise = util.createResolvable();
      const argsUi8 = encode(args);
      const buf = core_ts_6.core.dispatch(opId, argsUi8, zeroCopy);
      if (buf) {
        // Sync result.
        const res = decode(buf);
        promise.resolve(res);
      } else {
        // Async result.
        promiseTable[promiseId] = promise;
      }
      const res = await promise;
      return unwrapResponse(res);
    }
    exports_69("sendAsync", sendAsync);
    return {
      setters: [
        function (util_4) {
          util = util_4;
        },
        function (core_ts_6_1) {
          core_ts_6 = core_ts_6_1;
        },
        function (runtime_ts_5_1) {
          runtime_ts_5 = runtime_ts_5_1;
        },
        function (errors_ts_7_1) {
          errors_ts_7 = errors_ts_7_1;
        },
      ],
      execute: function () {
        // Using an object without a prototype because `Map` was causing GC problems.
        promiseTable = Object.create(null);
        _nextPromiseId = 1;
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/runtime_compiler.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_70, context_70) {
    "use strict";
    let dispatch_json_ts_33;
    const __moduleName = context_70 && context_70.id;
    function compile(request) {
      return dispatch_json_ts_33.sendAsync("op_compile", request);
    }
    exports_70("compile", compile);
    function transpile(request) {
      return dispatch_json_ts_33.sendAsync("op_transpile", request);
    }
    exports_70("transpile", transpile);
    return {
      setters: [
        function (dispatch_json_ts_33_1) {
          dispatch_json_ts_33 = dispatch_json_ts_33_1;
        },
      ],
      execute: function () {},
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/compiler/api.ts",
  ["$deno$/util.ts", "$deno$/ops/runtime_compiler.ts"],
  function (exports_71, context_71) {
    "use strict";
    let util, runtimeCompilerOps;
    const __moduleName = context_71 && context_71.id;
    function checkRelative(specifier) {
      return specifier.match(/^([\.\/\\]|https?:\/{2}|file:\/{2})/)
        ? specifier
        : `./${specifier}`;
    }
    async function transpileOnly(sources, options = {}) {
      util.log("Deno.transpileOnly", {
        sources: Object.keys(sources),
        options,
      });
      const payload = {
        sources,
        options: JSON.stringify(options),
      };
      const result = await runtimeCompilerOps.transpile(payload);
      return JSON.parse(result);
    }
    exports_71("transpileOnly", transpileOnly);
    async function compile(rootName, sources, options = {}) {
      const payload = {
        rootName: sources ? rootName : checkRelative(rootName),
        sources,
        options: JSON.stringify(options),
        bundle: false,
      };
      util.log("Deno.compile", {
        rootName: payload.rootName,
        sources: !!sources,
        options,
      });
      const result = await runtimeCompilerOps.compile(payload);
      return JSON.parse(result);
    }
    exports_71("compile", compile);
    async function bundle(rootName, sources, options = {}) {
      const payload = {
        rootName: sources ? rootName : checkRelative(rootName),
        sources,
        options: JSON.stringify(options),
        bundle: true,
      };
      util.log("Deno.bundle", {
        rootName: payload.rootName,
        sources: !!sources,
        options,
      });
      const result = await runtimeCompilerOps.compile(payload);
      return JSON.parse(result);
    }
    exports_71("bundle", bundle);
    return {
      setters: [
        function (util_5) {
          util = util_5;
        },
        function (runtimeCompilerOps_1) {
          runtimeCompilerOps = runtimeCompilerOps_1;
        },
      ],
      execute: function () {},
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/compiler/type_directives.ts", [], function (
  exports_72,
  context_72
) {
  "use strict";
  let typeDirectiveRegEx, importExportRegEx;
  const __moduleName = context_72 && context_72.id;
  function getMappedModuleName(source, typeDirectives) {
    const { fileName: sourceFileName, pos: sourcePos } = source;
    for (const [{ fileName, pos }, value] of typeDirectives.entries()) {
      if (sourceFileName === fileName && sourcePos === pos) {
        return value;
      }
    }
    return source.fileName;
  }
  exports_72("getMappedModuleName", getMappedModuleName);
  function parseTypeDirectives(sourceCode) {
    if (!sourceCode) {
      return;
    }
    // collect all the directives in the file and their start and end positions
    const directives = [];
    let maybeMatch = null;
    while ((maybeMatch = typeDirectiveRegEx.exec(sourceCode))) {
      const [matchString, , fileName] = maybeMatch;
      const { index: pos } = maybeMatch;
      directives.push({
        fileName,
        pos,
        end: pos + matchString.length,
      });
    }
    if (!directives.length) {
      return;
    }
    // work from the last directive backwards for the next `import`/`export`
    // statement
    directives.reverse();
    const results = new Map();
    for (const { end, fileName, pos } of directives) {
      const searchString = sourceCode.substring(end);
      const maybeMatch = importExportRegEx.exec(searchString);
      if (maybeMatch) {
        const [matchString, , targetFileName] = maybeMatch;
        const targetPos =
          end + maybeMatch.index + matchString.indexOf(targetFileName) - 1;
        const target = {
          fileName: targetFileName,
          pos: targetPos,
          end: targetPos + targetFileName.length,
        };
        results.set(target, fileName);
      }
      sourceCode = sourceCode.substring(0, pos);
    }
    return results;
  }
  exports_72("parseTypeDirectives", parseTypeDirectives);
  return {
    setters: [],
    execute: function () {
      typeDirectiveRegEx = /@deno-types\s*=\s*(["'])((?:(?=(\\?))\3.)*?)\1/gi;
      importExportRegEx = /(?:import|export)(?:\s+|\s+[\s\S]*?from\s+)?(["'])((?:(?=(\\?))\3.)*?)\1/;
    },
  };
});
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/compiler/sourcefile.ts",
  ["$deno$/compiler/type_directives.ts", "$deno$/util.ts"],
  function (exports_73, context_73) {
    "use strict";
    let type_directives_ts_1,
      util_ts_9,
      MediaType,
      moduleCache,
      specifierCache,
      SourceFile;
    const __moduleName = context_73 && context_73.id;
    function getExtension(fileName, mediaType) {
      switch (mediaType) {
        case MediaType.JavaScript:
          return ts.Extension.Js;
        case MediaType.JSX:
          return ts.Extension.Jsx;
        case MediaType.TypeScript:
          return fileName.endsWith(".d.ts")
            ? ts.Extension.Dts
            : ts.Extension.Ts;
        case MediaType.TSX:
          return ts.Extension.Tsx;
        case MediaType.Json:
          // we internally compile JSON, so what gets provided to the TypeScript
          // compiler is an ES module, but in order to get TypeScript to handle it
          // properly we have to pretend it is TS.
          return ts.Extension.Ts;
        case MediaType.Wasm:
          // Custom marker for Wasm type.
          return ts.Extension.Js;
        case MediaType.Unknown:
        default:
          throw TypeError(
            `Cannot resolve extension for "${fileName}" with mediaType "${MediaType[mediaType]}".`
          );
      }
    }
    return {
      setters: [
        function (type_directives_ts_1_1) {
          type_directives_ts_1 = type_directives_ts_1_1;
        },
        function (util_ts_9_1) {
          util_ts_9 = util_ts_9_1;
        },
      ],
      execute: function () {
        // Warning! The values in this enum are duplicated in `cli/msg.rs`
        // Update carefully!
        (function (MediaType) {
          MediaType[(MediaType["JavaScript"] = 0)] = "JavaScript";
          MediaType[(MediaType["JSX"] = 1)] = "JSX";
          MediaType[(MediaType["TypeScript"] = 2)] = "TypeScript";
          MediaType[(MediaType["TSX"] = 3)] = "TSX";
          MediaType[(MediaType["Json"] = 4)] = "Json";
          MediaType[(MediaType["Wasm"] = 5)] = "Wasm";
          MediaType[(MediaType["Unknown"] = 6)] = "Unknown";
        })(MediaType || (MediaType = {}));
        exports_73("MediaType", MediaType);
        exports_73("ASSETS", "$asset$");
        /** A global cache of module source files that have been loaded. */
        moduleCache = new Map();
        /** A map of maps which cache source files for quicker modules resolution. */
        specifierCache = new Map();
        SourceFile = class SourceFile {
          constructor(json) {
            this.processed = false;
            if (moduleCache.has(json.url)) {
              throw new TypeError("SourceFile already exists");
            }
            Object.assign(this, json);
            this.extension = getExtension(this.url, this.mediaType);
            moduleCache.set(this.url, this);
          }
          cache(moduleSpecifier, containingFile) {
            containingFile = containingFile || "";
            let innerCache = specifierCache.get(containingFile);
            if (!innerCache) {
              innerCache = new Map();
              specifierCache.set(containingFile, innerCache);
            }
            innerCache.set(moduleSpecifier, this);
          }
          imports(processJsImports) {
            if (this.processed) {
              throw new Error("SourceFile has already been processed.");
            }
            util_ts_9.assert(this.sourceCode != null);
            // we shouldn't process imports for files which contain the nocheck pragma
            // (like bundles)
            if (this.sourceCode.match(/\/{2}\s+@ts-nocheck/)) {
              util_ts_9.log(`Skipping imports for "${this.filename}"`);
              return [];
            }
            const preProcessedFileInfo = ts.preProcessFile(
              this.sourceCode,
              true,
              this.mediaType === MediaType.JavaScript ||
                this.mediaType === MediaType.JSX
            );
            this.processed = true;
            const files = (this.importedFiles = []);
            function process(references) {
              for (const { fileName } of references) {
                files.push([fileName, fileName]);
              }
            }
            const {
              importedFiles,
              referencedFiles,
              libReferenceDirectives,
              typeReferenceDirectives,
            } = preProcessedFileInfo;
            const typeDirectives = type_directives_ts_1.parseTypeDirectives(
              this.sourceCode
            );
            if (typeDirectives) {
              for (const importedFile of importedFiles) {
                files.push([
                  importedFile.fileName,
                  type_directives_ts_1.getMappedModuleName(
                    importedFile,
                    typeDirectives
                  ),
                ]);
              }
            } else if (
              !(
                !processJsImports &&
                (this.mediaType === MediaType.JavaScript ||
                  this.mediaType === MediaType.JSX)
              )
            ) {
              process(importedFiles);
            }
            process(referencedFiles);
            // built in libs comes across as `"dom"` for example, and should be filtered
            // out during pre-processing as they are either already cached or they will
            // be lazily fetched by the compiler host.  Ones that contain full files are
            // not filtered out and will be fetched as normal.
            process(
              libReferenceDirectives.filter(
                ({ fileName }) => !ts.libMap.has(fileName.toLowerCase())
              )
            );
            process(typeReferenceDirectives);
            return files;
          }
          static getUrl(moduleSpecifier, containingFile) {
            const containingCache = specifierCache.get(containingFile);
            if (containingCache) {
              const sourceFile = containingCache.get(moduleSpecifier);
              return sourceFile && sourceFile.url;
            }
            return undefined;
          }
          static get(url) {
            return moduleCache.get(url);
          }
          static has(url) {
            return moduleCache.has(url);
          }
        };
        exports_73("SourceFile", SourceFile);
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/compiler/bundler.ts",
  ["$deno$/compiler/bootstrap.ts", "$deno$/compiler/util.ts", "$deno$/util.ts"],
  function (exports_74, context_74) {
    "use strict";
    let bootstrap_ts_1, util_ts_10, util_ts_11, rootExports;
    const __moduleName = context_74 && context_74.id;
    function normalizeUrl(rootName) {
      const match = /^(\S+:\/{2,3})(.+)$/.exec(rootName);
      if (match) {
        const [, protocol, path] = match;
        return `${protocol}${util_ts_10.normalizeString(
          path,
          false,
          "/",
          (code) => code === util_ts_10.CHAR_FORWARD_SLASH
        )}`;
      } else {
        return rootName;
      }
    }
    function buildBundle(rootName, data, sourceFiles) {
      // when outputting to AMD and a single outfile, TypeScript makes up the module
      // specifiers which are used to define the modules, and doesn't expose them
      // publicly, so we have to try to replicate
      const sources = sourceFiles.map((sf) => sf.fileName);
      const sharedPath = util_ts_10.commonPath(sources);
      rootName = normalizeUrl(rootName)
        .replace(sharedPath, "")
        .replace(/\.\w+$/i, "");
      // If one of the modules requires support for top-level-await, TypeScript will
      // emit the execute function as an async function.  When this is the case we
      // need to bubble up the TLA to the instantiation, otherwise we instantiate
      // synchronously.
      const hasTla = data.match(/execute:\sasync\sfunction\s/);
      let instantiate;
      if (rootExports && rootExports.length) {
        instantiate = hasTla
          ? `const __exp = await __instantiateAsync("${rootName}");\n`
          : `const __exp = __instantiate("${rootName}");\n`;
        for (const rootExport of rootExports) {
          if (rootExport === "default") {
            instantiate += `export default __exp["${rootExport}"];\n`;
          } else {
            instantiate += `export const ${rootExport} = __exp["${rootExport}"];\n`;
          }
        }
      } else {
        instantiate = hasTla
          ? `await __instantiateAsync("${rootName}");\n`
          : `__instantiate("${rootName}");\n`;
      }
      return `${bootstrap_ts_1.SYSTEM_LOADER}\n${data}\n${instantiate}`;
    }
    exports_74("buildBundle", buildBundle);
    function setRootExports(program, rootModule) {
      // get a reference to the type checker, this will let us find symbols from
      // the AST.
      const checker = program.getTypeChecker();
      // get a reference to the main source file for the bundle
      const mainSourceFile = program.getSourceFile(rootModule);
      util_ts_11.assert(mainSourceFile);
      // retrieve the internal TypeScript symbol for this AST node
      const mainSymbol = checker.getSymbolAtLocation(mainSourceFile);
      if (!mainSymbol) {
        return;
      }
      rootExports = checker
        .getExportsOfModule(mainSymbol)
        // .getExportsOfModule includes type only symbols which are exported from
        // the module, so we need to try to filter those out.  While not critical
        // someone looking at the bundle would think there is runtime code behind
        // that when there isn't.  There appears to be no clean way of figuring that
        // out, so inspecting SymbolFlags that might be present that are type only
        .filter(
          (sym) =>
            sym.flags & ts.SymbolFlags.Class ||
            !(
              sym.flags & ts.SymbolFlags.Interface ||
              sym.flags & ts.SymbolFlags.TypeLiteral ||
              sym.flags & ts.SymbolFlags.Signature ||
              sym.flags & ts.SymbolFlags.TypeParameter ||
              sym.flags & ts.SymbolFlags.TypeAlias ||
              sym.flags & ts.SymbolFlags.Type ||
              sym.flags & ts.SymbolFlags.Namespace ||
              sym.flags & ts.SymbolFlags.InterfaceExcludes ||
              sym.flags & ts.SymbolFlags.TypeParameterExcludes ||
              sym.flags & ts.SymbolFlags.TypeAliasExcludes
            )
        )
        .map((sym) => sym.getName());
    }
    exports_74("setRootExports", setRootExports);
    return {
      setters: [
        function (bootstrap_ts_1_1) {
          bootstrap_ts_1 = bootstrap_ts_1_1;
        },
        function (util_ts_10_1) {
          util_ts_10 = util_ts_10_1;
        },
        function (util_ts_11_1) {
          util_ts_11 = util_ts_11_1;
        },
      ],
      execute: function () {},
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/compiler.ts",
  [
    "$deno$/ops/dispatch_json.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/core.ts",
  ],
  function (exports_75, context_75) {
    "use strict";
    let dispatch_json_ts_34, text_encoding_ts_6, core_ts_7, encoder, decoder;
    const __moduleName = context_75 && context_75.id;
    function resolveModules(specifiers, referrer) {
      return dispatch_json_ts_34.sendSync("op_resolve_modules", {
        specifiers,
        referrer,
      });
    }
    exports_75("resolveModules", resolveModules);
    function fetchSourceFiles(specifiers, referrer) {
      return dispatch_json_ts_34.sendAsync("op_fetch_source_files", {
        specifiers,
        referrer,
      });
    }
    exports_75("fetchSourceFiles", fetchSourceFiles);
    function getAsset(name) {
      const opId = core_ts_7.core.ops()["op_fetch_asset"];
      // We really don't want to depend on JSON dispatch during snapshotting, so
      // this op exchanges strings with Rust as raw byte arrays.
      const sourceCodeBytes = core_ts_7.core.dispatch(
        opId,
        encoder.encode(name)
      );
      return decoder.decode(sourceCodeBytes);
    }
    exports_75("getAsset", getAsset);
    function cache(extension, moduleId, contents) {
      dispatch_json_ts_34.sendSync("op_cache", {
        extension,
        moduleId,
        contents,
      });
    }
    exports_75("cache", cache);
    return {
      setters: [
        function (dispatch_json_ts_34_1) {
          dispatch_json_ts_34 = dispatch_json_ts_34_1;
        },
        function (text_encoding_ts_6_1) {
          text_encoding_ts_6 = text_encoding_ts_6_1;
        },
        function (core_ts_7_1) {
          core_ts_7 = core_ts_7_1;
        },
      ],
      execute: function () {
        encoder = new text_encoding_ts_6.TextEncoder();
        decoder = new text_encoding_ts_6.TextDecoder();
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/compiler/util.ts",
  [
    "$deno$/colors.ts",
    "$deno$/compiler/bundler.ts",
    "$deno$/compiler/sourcefile.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/ops/compiler.ts",
    "$deno$/util.ts",
    "$deno$/write_file.ts",
  ],
  function (exports_76, context_76) {
    "use strict";
    let colors_ts_2,
      bundler_ts_1,
      sourcefile_ts_1,
      text_encoding_ts_7,
      compilerOps,
      util,
      util_ts_12,
      write_file_ts_2,
      CompilerRequestType,
      OUT_DIR,
      CHAR_DOT,
      CHAR_FORWARD_SLASH;
    const __moduleName = context_76 && context_76.id;
    function cache(moduleId, emittedFileName, contents, checkJs = false) {
      util.log("compiler::cache", { moduleId, emittedFileName, checkJs });
      const sf = sourcefile_ts_1.SourceFile.get(moduleId);
      if (sf) {
        // NOTE: If it's a `.json` file we don't want to write it to disk.
        // JSON files are loaded and used by TS compiler to check types, but we don't want
        // to emit them to disk because output file is the same as input file.
        if (sf.mediaType === sourcefile_ts_1.MediaType.Json) {
          return;
        }
        // NOTE: JavaScript files are only cached to disk if `checkJs`
        // option in on
        if (sf.mediaType === sourcefile_ts_1.MediaType.JavaScript && !checkJs) {
          return;
        }
      }
      if (emittedFileName.endsWith(".map")) {
        // Source Map
        compilerOps.cache(".map", moduleId, contents);
      } else if (
        emittedFileName.endsWith(".js") ||
        emittedFileName.endsWith(".json")
      ) {
        // Compiled JavaScript
        compilerOps.cache(".js", moduleId, contents);
      } else {
        util_ts_12.assert(
          false,
          `Trying to cache unhandled file type "${emittedFileName}"`
        );
      }
    }
    function getAsset(name) {
      return compilerOps.getAsset(name);
    }
    exports_76("getAsset", getAsset);
    function createWriteFile(state) {
      const encoder = new text_encoding_ts_7.TextEncoder();
      if (state.type === CompilerRequestType.Compile) {
        return function writeFile(fileName, data, sourceFiles) {
          util_ts_12.assert(
            sourceFiles != null,
            `Unexpected emit of "${fileName}" which isn't part of a program.`
          );
          util_ts_12.assert(state.host);
          if (!state.bundle) {
            util_ts_12.assert(sourceFiles.length === 1);
            cache(
              sourceFiles[0].fileName,
              fileName,
              data,
              state.host.getCompilationSettings().checkJs
            );
          } else {
            // if the fileName is set to an internal value, just noop, this is
            // used in the Rust unit tests.
            if (state.outFile && state.outFile.startsWith(OUT_DIR)) {
              return;
            }
            // we only support single root names for bundles
            util_ts_12.assert(
              state.rootNames.length === 1,
              `Only one root name supported.  Got "${JSON.stringify(
                state.rootNames
              )}"`
            );
            // this enriches the string with the loader and re-exports the
            // exports of the root module
            const content = bundler_ts_1.buildBundle(
              state.rootNames[0],
              data,
              sourceFiles
            );
            if (state.outFile) {
              const encodedData = encoder.encode(content);
              console.warn(`Emitting bundle to "${state.outFile}"`);
              write_file_ts_2.writeFileSync(state.outFile, encodedData);
              console.warn(`${humanFileSize(encodedData.length)} emitted.`);
            } else {
              console.log(content);
            }
          }
        };
      }
      return function writeFile(fileName, data, sourceFiles) {
        util_ts_12.assert(sourceFiles != null);
        util_ts_12.assert(state.host);
        util_ts_12.assert(state.emitMap);
        if (!state.bundle) {
          util_ts_12.assert(sourceFiles.length === 1);
          state.emitMap[fileName] = data;
          // we only want to cache the compiler output if we are resolving
          // modules externally
          if (!state.sources) {
            cache(
              sourceFiles[0].fileName,
              fileName,
              data,
              state.host.getCompilationSettings().checkJs
            );
          }
        } else {
          // we only support single root names for bundles
          util_ts_12.assert(state.rootNames.length === 1);
          state.emitBundle = bundler_ts_1.buildBundle(
            state.rootNames[0],
            data,
            sourceFiles
          );
        }
      };
    }
    exports_76("createWriteFile", createWriteFile);
    function convertCompilerOptions(str) {
      const options = JSON.parse(str);
      const out = {};
      const keys = Object.keys(options);
      const files = [];
      for (const key of keys) {
        switch (key) {
          case "jsx":
            const value = options[key];
            if (value === "preserve") {
              out[key] = ts.JsxEmit.Preserve;
            } else if (value === "react") {
              out[key] = ts.JsxEmit.React;
            } else {
              out[key] = ts.JsxEmit.ReactNative;
            }
            break;
          case "module":
            switch (options[key]) {
              case "amd":
                out[key] = ts.ModuleKind.AMD;
                break;
              case "commonjs":
                out[key] = ts.ModuleKind.CommonJS;
                break;
              case "es2015":
              case "es6":
                out[key] = ts.ModuleKind.ES2015;
                break;
              case "esnext":
                out[key] = ts.ModuleKind.ESNext;
                break;
              case "none":
                out[key] = ts.ModuleKind.None;
                break;
              case "system":
                out[key] = ts.ModuleKind.System;
                break;
              case "umd":
                out[key] = ts.ModuleKind.UMD;
                break;
              default:
                throw new TypeError("Unexpected module type");
            }
            break;
          case "target":
            switch (options[key]) {
              case "es3":
                out[key] = ts.ScriptTarget.ES3;
                break;
              case "es5":
                out[key] = ts.ScriptTarget.ES5;
                break;
              case "es6":
              case "es2015":
                out[key] = ts.ScriptTarget.ES2015;
                break;
              case "es2016":
                out[key] = ts.ScriptTarget.ES2016;
                break;
              case "es2017":
                out[key] = ts.ScriptTarget.ES2017;
                break;
              case "es2018":
                out[key] = ts.ScriptTarget.ES2018;
                break;
              case "es2019":
                out[key] = ts.ScriptTarget.ES2019;
                break;
              case "es2020":
                out[key] = ts.ScriptTarget.ES2020;
                break;
              case "esnext":
                out[key] = ts.ScriptTarget.ESNext;
                break;
              default:
                throw new TypeError("Unexpected emit target.");
            }
            break;
          case "types":
            const types = options[key];
            util_ts_12.assert(types);
            files.push(...types);
            break;
          default:
            out[key] = options[key];
        }
      }
      return {
        options: out,
        files: files.length ? files : undefined,
      };
    }
    exports_76("convertCompilerOptions", convertCompilerOptions);
    function processConfigureResponse(configResult, configPath) {
      const { ignoredOptions, diagnostics } = configResult;
      if (ignoredOptions) {
        console.warn(
          colors_ts_2.yellow(
            `Unsupported compiler options in "${configPath}"\n`
          ) +
            colors_ts_2.cyan(`  The following options were ignored:\n`) +
            `    ${ignoredOptions
              .map((value) => colors_ts_2.bold(value))
              .join(", ")}`
        );
      }
      return diagnostics;
    }
    exports_76("processConfigureResponse", processConfigureResponse);
    function normalizeString(path, allowAboveRoot, separator, isPathSeparator) {
      let res = "";
      let lastSegmentLength = 0;
      let lastSlash = -1;
      let dots = 0;
      let code;
      for (let i = 0, len = path.length; i <= len; ++i) {
        if (i < len) code = path.charCodeAt(i);
        else if (isPathSeparator(code)) break;
        else code = CHAR_FORWARD_SLASH;
        if (isPathSeparator(code)) {
          if (lastSlash === i - 1 || dots === 1) {
            // NOOP
          } else if (lastSlash !== i - 1 && dots === 2) {
            if (
              res.length < 2 ||
              lastSegmentLength !== 2 ||
              res.charCodeAt(res.length - 1) !== CHAR_DOT ||
              res.charCodeAt(res.length - 2) !== CHAR_DOT
            ) {
              if (res.length > 2) {
                const lastSlashIndex = res.lastIndexOf(separator);
                if (lastSlashIndex === -1) {
                  res = "";
                  lastSegmentLength = 0;
                } else {
                  res = res.slice(0, lastSlashIndex);
                  lastSegmentLength =
                    res.length - 1 - res.lastIndexOf(separator);
                }
                lastSlash = i;
                dots = 0;
                continue;
              } else if (res.length === 2 || res.length === 1) {
                res = "";
                lastSegmentLength = 0;
                lastSlash = i;
                dots = 0;
                continue;
              }
            }
            if (allowAboveRoot) {
              if (res.length > 0) res += `${separator}..`;
              else res = "..";
              lastSegmentLength = 2;
            }
          } else {
            if (res.length > 0) res += separator + path.slice(lastSlash + 1, i);
            else res = path.slice(lastSlash + 1, i);
            lastSegmentLength = i - lastSlash - 1;
          }
          lastSlash = i;
          dots = 0;
        } else if (code === CHAR_DOT && dots !== -1) {
          ++dots;
        } else {
          dots = -1;
        }
      }
      return res;
    }
    exports_76("normalizeString", normalizeString);
    function commonPath(paths, sep = "/") {
      const [first = "", ...remaining] = paths;
      if (first === "" || remaining.length === 0) {
        return first.substring(0, first.lastIndexOf(sep) + 1);
      }
      const parts = first.split(sep);
      let endOfPrefix = parts.length;
      for (const path of remaining) {
        const compare = path.split(sep);
        for (let i = 0; i < endOfPrefix; i++) {
          if (compare[i] !== parts[i]) {
            endOfPrefix = i;
          }
        }
        if (endOfPrefix === 0) {
          return "";
        }
      }
      const prefix = parts.slice(0, endOfPrefix).join(sep);
      return prefix.endsWith(sep) ? prefix : `${prefix}${sep}`;
    }
    exports_76("commonPath", commonPath);
    function humanFileSize(bytes) {
      const thresh = 1000;
      if (Math.abs(bytes) < thresh) {
        return bytes + " B";
      }
      const units = ["kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
      let u = -1;
      do {
        bytes /= thresh;
        ++u;
      } while (Math.abs(bytes) >= thresh && u < units.length - 1);
      return `${bytes.toFixed(1)} ${units[u]}`;
    }
    // @internal
    function base64ToUint8Array(data) {
      const binString = text_encoding_ts_7.atob(data);
      const size = binString.length;
      const bytes = new Uint8Array(size);
      for (let i = 0; i < size; i++) {
        bytes[i] = binString.charCodeAt(i);
      }
      return bytes;
    }
    exports_76("base64ToUint8Array", base64ToUint8Array);
    return {
      setters: [
        function (colors_ts_2_1) {
          colors_ts_2 = colors_ts_2_1;
        },
        function (bundler_ts_1_1) {
          bundler_ts_1 = bundler_ts_1_1;
        },
        function (sourcefile_ts_1_1) {
          sourcefile_ts_1 = sourcefile_ts_1_1;
        },
        function (text_encoding_ts_7_1) {
          text_encoding_ts_7 = text_encoding_ts_7_1;
        },
        function (compilerOps_1) {
          compilerOps = compilerOps_1;
        },
        function (util_6) {
          util = util_6;
          util_ts_12 = util_6;
        },
        function (write_file_ts_2_1) {
          write_file_ts_2 = write_file_ts_2_1;
        },
      ],
      execute: function () {
        // Warning! The values in this enum are duplicated in `cli/msg.rs`
        // Update carefully!
        (function (CompilerRequestType) {
          CompilerRequestType[(CompilerRequestType["Compile"] = 0)] = "Compile";
          CompilerRequestType[(CompilerRequestType["RuntimeCompile"] = 1)] =
            "RuntimeCompile";
          CompilerRequestType[(CompilerRequestType["RuntimeTranspile"] = 2)] =
            "RuntimeTranspile";
        })(CompilerRequestType || (CompilerRequestType = {}));
        exports_76("CompilerRequestType", CompilerRequestType);
        exports_76("OUT_DIR", (OUT_DIR = "$deno$"));
        exports_76("ignoredDiagnostics", [
          // TS2306: File 'cli/tests/subdir/amd_like.js' is
          // not a module.
          2306,
          // TS1375: 'await' expressions are only allowed at the top level of a file
          // when that file is a module, but this file has no imports or exports.
          // Consider adding an empty 'export {}' to make this file a module.
          1375,
          // TS1103: 'for-await-of' statement is only allowed within an async function
          // or async generator.
          1103,
          // TS2691: An import path cannot end with a '.ts' extension. Consider
          // importing 'bad-module' instead.
          2691,
          // TS5009: Cannot find the common subdirectory path for the input files.
          5009,
          // TS5055: Cannot write file
          // 'http://localhost:4545/cli/tests/subdir/mt_application_x_javascript.j4.js'
          // because it would overwrite input file.
          5055,
          // TypeScript is overly opinionated that only CommonJS modules kinds can
          // support JSON imports.  Allegedly this was fixed in
          // Microsoft/TypeScript#26825 but that doesn't seem to be working here,
          // so we will ignore complaints about this compiler setting.
          5070,
          // TS7016: Could not find a declaration file for module '...'. '...'
          // implicitly has an 'any' type.  This is due to `allowJs` being off by
          // default but importing of a JavaScript module.
          7016,
        ]);
        // Constants used by `normalizeString` and `resolvePath`
        exports_76("CHAR_DOT", (CHAR_DOT = 46)); /* . */
        exports_76("CHAR_FORWARD_SLASH", (CHAR_FORWARD_SLASH = 47)); /* / */
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/compiler/host.ts",
  [
    "$deno$/compiler/sourcefile.ts",
    "$deno$/compiler/util.ts",
    "$deno$/ops/fs/dir.ts",
    "$deno$/util.ts",
  ],
  function (exports_77, context_77) {
    "use strict";
    let sourcefile_ts_2,
      util_ts_13,
      dir_ts_2,
      util_ts_14,
      util,
      CompilerHostTarget,
      defaultBundlerOptions,
      defaultCompileOptions,
      ignoredCompilerOptions,
      Host;
    const __moduleName = context_77 && context_77.id;
    function getAssetInternal(filename) {
      const lastSegment = filename.split("/").pop();
      const url = ts.libMap.has(lastSegment)
        ? ts.libMap.get(lastSegment)
        : lastSegment;
      const sourceFile = sourcefile_ts_2.SourceFile.get(url);
      if (sourceFile) {
        return sourceFile;
      }
      const name = url.includes(".") ? url : `${url}.d.ts`;
      const sourceCode = util_ts_13.getAsset(name);
      return new sourcefile_ts_2.SourceFile({
        url,
        filename: `${sourcefile_ts_2.ASSETS}/${name}`,
        mediaType: sourcefile_ts_2.MediaType.TypeScript,
        sourceCode,
      });
    }
    return {
      setters: [
        function (sourcefile_ts_2_1) {
          sourcefile_ts_2 = sourcefile_ts_2_1;
        },
        function (util_ts_13_1) {
          util_ts_13 = util_ts_13_1;
        },
        function (dir_ts_2_1) {
          dir_ts_2 = dir_ts_2_1;
        },
        function (util_ts_14_1) {
          util_ts_14 = util_ts_14_1;
          util = util_ts_14_1;
        },
      ],
      execute: function () {
        (function (CompilerHostTarget) {
          CompilerHostTarget["Main"] = "main";
          CompilerHostTarget["Runtime"] = "runtime";
          CompilerHostTarget["Worker"] = "worker";
        })(CompilerHostTarget || (CompilerHostTarget = {}));
        exports_77("CompilerHostTarget", CompilerHostTarget);
        exports_77(
          "defaultBundlerOptions",
          (defaultBundlerOptions = {
            allowJs: true,
            inlineSourceMap: false,
            module: ts.ModuleKind.System,
            outDir: undefined,
            outFile: `${util_ts_13.OUT_DIR}/bundle.js`,
            // disabled until we have effective way to modify source maps
            sourceMap: false,
          })
        );
        exports_77(
          "defaultCompileOptions",
          (defaultCompileOptions = {
            allowJs: false,
            allowNonTsExtensions: true,
            checkJs: false,
            esModuleInterop: true,
            jsx: ts.JsxEmit.React,
            module: ts.ModuleKind.ESNext,
            outDir: util_ts_13.OUT_DIR,
            resolveJsonModule: true,
            sourceMap: true,
            strict: true,
            stripComments: true,
            target: ts.ScriptTarget.ESNext,
          })
        );
        exports_77("defaultRuntimeCompileOptions", {
          outDir: undefined,
        });
        exports_77("defaultTranspileOptions", {
          esModuleInterop: true,
          module: ts.ModuleKind.ESNext,
          sourceMap: true,
          scriptComments: true,
          target: ts.ScriptTarget.ESNext,
        });
        ignoredCompilerOptions = [
          "allowSyntheticDefaultImports",
          "baseUrl",
          "build",
          "composite",
          "declaration",
          "declarationDir",
          "declarationMap",
          "diagnostics",
          "downlevelIteration",
          "emitBOM",
          "emitDeclarationOnly",
          "esModuleInterop",
          "extendedDiagnostics",
          "forceConsistentCasingInFileNames",
          "help",
          "importHelpers",
          "incremental",
          "inlineSourceMap",
          "inlineSources",
          "init",
          "isolatedModules",
          "listEmittedFiles",
          "listFiles",
          "mapRoot",
          "maxNodeModuleJsDepth",
          "module",
          "moduleResolution",
          "newLine",
          "noEmit",
          "noEmitHelpers",
          "noEmitOnError",
          "noLib",
          "noResolve",
          "out",
          "outDir",
          "outFile",
          "paths",
          "preserveSymlinks",
          "preserveWatchOutput",
          "pretty",
          "rootDir",
          "rootDirs",
          "showConfig",
          "skipDefaultLibCheck",
          "skipLibCheck",
          "sourceMap",
          "sourceRoot",
          "stripInternal",
          "target",
          "traceResolution",
          "tsBuildInfoFile",
          "types",
          "typeRoots",
          "version",
          "watch",
        ];
        Host = class Host {
          /* Deno specific APIs */
          constructor({ bundle = false, target, writeFile }) {
            this.#options = defaultCompileOptions;
            this.#target = target;
            this.#writeFile = writeFile;
            if (bundle) {
              // options we need to change when we are generating a bundle
              Object.assign(this.#options, defaultBundlerOptions);
            }
          }
          #options;
          #target;
          #writeFile;
          configure(path, configurationText) {
            util.log("compiler::host.configure", path);
            util_ts_14.assert(configurationText);
            const { config, error } = ts.parseConfigFileTextToJson(
              path,
              configurationText
            );
            if (error) {
              return { diagnostics: [error] };
            }
            const { options, errors } = ts.convertCompilerOptionsFromJson(
              config.compilerOptions,
              dir_ts_2.cwd()
            );
            const ignoredOptions = [];
            for (const key of Object.keys(options)) {
              if (
                ignoredCompilerOptions.includes(key) &&
                (!(key in this.#options) || options[key] !== this.#options[key])
              ) {
                ignoredOptions.push(key);
                delete options[key];
              }
            }
            Object.assign(this.#options, options);
            return {
              ignoredOptions: ignoredOptions.length
                ? ignoredOptions
                : undefined,
              diagnostics: errors.length ? errors : undefined,
            };
          }
          mergeOptions(...options) {
            Object.assign(this.#options, ...options);
            return Object.assign({}, this.#options);
          }
          /* TypeScript CompilerHost APIs */
          fileExists(_fileName) {
            return util_ts_14.notImplemented();
          }
          getCanonicalFileName(fileName) {
            return fileName;
          }
          getCompilationSettings() {
            util.log("compiler::host.getCompilationSettings()");
            return this.#options;
          }
          getCurrentDirectory() {
            return "";
          }
          getDefaultLibFileName(_options) {
            util.log("compiler::host.getDefaultLibFileName()");
            switch (this.#target) {
              case CompilerHostTarget.Main:
              case CompilerHostTarget.Runtime:
                return `${sourcefile_ts_2.ASSETS}/lib.deno.window.d.ts`;
              case CompilerHostTarget.Worker:
                return `${sourcefile_ts_2.ASSETS}/lib.deno.worker.d.ts`;
            }
          }
          getNewLine() {
            return "\n";
          }
          getSourceFile(
            fileName,
            languageVersion,
            onError,
            shouldCreateNewSourceFile
          ) {
            util.log("compiler::host.getSourceFile", fileName);
            try {
              util_ts_14.assert(!shouldCreateNewSourceFile);
              const sourceFile = fileName.startsWith(sourcefile_ts_2.ASSETS)
                ? getAssetInternal(fileName)
                : sourcefile_ts_2.SourceFile.get(fileName);
              util_ts_14.assert(sourceFile != null);
              if (!sourceFile.tsSourceFile) {
                util_ts_14.assert(sourceFile.sourceCode != null);
                // even though we assert the extension for JSON modules to the compiler
                // is TypeScript, TypeScript internally analyses the filename for its
                // extension and tries to parse it as JSON instead of TS.  We have to
                // change the filename to the TypeScript file.
                sourceFile.tsSourceFile = ts.createSourceFile(
                  fileName.startsWith(sourcefile_ts_2.ASSETS)
                    ? sourceFile.filename
                    : fileName.toLowerCase().endsWith(".json")
                    ? `${fileName}.ts`
                    : fileName,
                  sourceFile.sourceCode,
                  languageVersion
                );
                delete sourceFile.sourceCode;
              }
              return sourceFile.tsSourceFile;
            } catch (e) {
              if (onError) {
                onError(String(e));
              } else {
                throw e;
              }
              return undefined;
            }
          }
          readFile(_fileName) {
            return util_ts_14.notImplemented();
          }
          resolveModuleNames(moduleNames, containingFile) {
            util.log("compiler::host.resolveModuleNames", {
              moduleNames,
              containingFile,
            });
            return moduleNames.map((specifier) => {
              const url = sourcefile_ts_2.SourceFile.getUrl(
                specifier,
                containingFile
              );
              const sourceFile = specifier.startsWith(sourcefile_ts_2.ASSETS)
                ? getAssetInternal(specifier)
                : url
                ? sourcefile_ts_2.SourceFile.get(url)
                : undefined;
              if (!sourceFile) {
                return undefined;
              }
              return {
                resolvedFileName: sourceFile.url,
                isExternalLibraryImport: specifier.startsWith(
                  sourcefile_ts_2.ASSETS
                ),
                extension: sourceFile.extension,
              };
            });
          }
          useCaseSensitiveFileNames() {
            return true;
          }
          writeFile(
            fileName,
            data,
            _writeByteOrderMark,
            _onError,
            sourceFiles
          ) {
            util.log("compiler::host.writeFile", fileName);
            this.#writeFile(fileName, data, sourceFiles);
          }
        };
        exports_77("Host", Host);
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/compiler/bootstrap.ts",
  [
    "$deno$/compiler/host.ts",
    "$deno$/compiler/sourcefile.ts",
    "$deno$/compiler/util.ts",
  ],
  function (exports_78, context_78) {
    "use strict";
    let host_ts_1, sourcefile_ts_3, util_ts_15, host, options;
    const __moduleName = context_78 && context_78.id;
    return {
      setters: [
        function (host_ts_1_1) {
          host_ts_1 = host_ts_1_1;
        },
        function (sourcefile_ts_3_1) {
          sourcefile_ts_3 = sourcefile_ts_3_1;
        },
        function (util_ts_15_1) {
          util_ts_15 = util_ts_15_1;
        },
      ],
      execute: function () {
        // NOTE: target doesn't really matter here,
        // this is in fact a mock host created just to
        // load all type definitions and snapshot them.
        host = new host_ts_1.Host({
          target: host_ts_1.CompilerHostTarget.Main,
          writeFile() {},
        });
        options = host.getCompilationSettings();
        // This is a hacky way of adding our libs to the libs available in TypeScript()
        // as these are internal APIs of TypeScript which maintain valid libs
        ts.libs.push(
          "deno.ns",
          "deno.window",
          "deno.worker",
          "deno.shared_globals"
        );
        ts.libMap.set("deno.ns", "lib.deno.ns.d.ts");
        ts.libMap.set("deno.window", "lib.deno.window.d.ts");
        ts.libMap.set("deno.worker", "lib.deno.worker.d.ts");
        ts.libMap.set("deno.shared_globals", "lib.deno.shared_globals.d.ts");
        // this pre-populates the cache at snapshot time of our library files, so they
        // are available in the future when needed.
        host.getSourceFile(
          `${sourcefile_ts_3.ASSETS}/lib.deno.ns.d.ts`,
          ts.ScriptTarget.ESNext
        );
        host.getSourceFile(
          `${sourcefile_ts_3.ASSETS}/lib.deno.window.d.ts`,
          ts.ScriptTarget.ESNext
        );
        host.getSourceFile(
          `${sourcefile_ts_3.ASSETS}/lib.deno.worker.d.ts`,
          ts.ScriptTarget.ESNext
        );
        host.getSourceFile(
          `${sourcefile_ts_3.ASSETS}/lib.deno.shared_globals.d.ts`,
          ts.ScriptTarget.ESNext
        );
        exports_78(
          "TS_SNAPSHOT_PROGRAM",
          ts.createProgram({
            rootNames: [`${sourcefile_ts_3.ASSETS}/bootstrap.ts`],
            options,
            host,
          })
        );
        exports_78("SYSTEM_LOADER", util_ts_15.getAsset("system_loader.js"));
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/compiler/imports.ts",
  [
    "$deno$/compiler/sourcefile.ts",
    "$deno$/compiler/util.ts",
    "$deno$/ops/fs/dir.ts",
    "$deno$/util.ts",
    "$deno$/ops/compiler.ts",
  ],
  function (exports_79, context_79) {
    "use strict";
    let sourcefile_ts_4, util_ts_16, dir_ts_3, util_ts_17, util, compilerOps;
    const __moduleName = context_79 && context_79.id;
    function resolvePath(...pathSegments) {
      let resolvedPath = "";
      let resolvedAbsolute = false;
      for (let i = pathSegments.length - 1; i >= -1 && !resolvedAbsolute; i--) {
        let path;
        if (i >= 0) path = pathSegments[i];
        else path = dir_ts_3.cwd();
        // Skip empty entries
        if (path.length === 0) {
          continue;
        }
        resolvedPath = `${path}/${resolvedPath}`;
        resolvedAbsolute = path.charCodeAt(0) === util_ts_16.CHAR_FORWARD_SLASH;
      }
      // At this point the path should be resolved to a full absolute path, but
      // handle relative paths to be safe (might happen when cwd() fails)
      // Normalize the path
      resolvedPath = util_ts_16.normalizeString(
        resolvedPath,
        !resolvedAbsolute,
        "/",
        (code) => code === util_ts_16.CHAR_FORWARD_SLASH
      );
      if (resolvedAbsolute) {
        if (resolvedPath.length > 0) return `/${resolvedPath}`;
        else return "/";
      } else if (resolvedPath.length > 0) return resolvedPath;
      else return ".";
    }
    function resolveSpecifier(specifier, referrer) {
      if (!specifier.startsWith(".")) {
        return specifier;
      }
      const pathParts = referrer.split("/");
      pathParts.pop();
      let path = pathParts.join("/");
      path = path.endsWith("/") ? path : `${path}/`;
      return resolvePath(path, specifier);
    }
    function resolveModules(specifiers, referrer) {
      util.log("compiler_imports::resolveModules", { specifiers, referrer });
      return compilerOps.resolveModules(specifiers, referrer);
    }
    exports_79("resolveModules", resolveModules);
    function fetchSourceFiles(specifiers, referrer) {
      util.log("compiler_imports::fetchSourceFiles", { specifiers, referrer });
      return compilerOps.fetchSourceFiles(specifiers, referrer);
    }
    function getMediaType(filename) {
      const maybeExtension = /\.([a-zA-Z]+)$/.exec(filename);
      if (!maybeExtension) {
        util.log(`!!! Could not identify valid extension: "${filename}"`);
        return sourcefile_ts_4.MediaType.Unknown;
      }
      const [, extension] = maybeExtension;
      switch (extension.toLowerCase()) {
        case "js":
          return sourcefile_ts_4.MediaType.JavaScript;
        case "jsx":
          return sourcefile_ts_4.MediaType.JSX;
        case "json":
          return sourcefile_ts_4.MediaType.Json;
        case "ts":
          return sourcefile_ts_4.MediaType.TypeScript;
        case "tsx":
          return sourcefile_ts_4.MediaType.TSX;
        case "wasm":
          return sourcefile_ts_4.MediaType.Wasm;
        default:
          util.log(`!!! Unknown extension: "${extension}"`);
          return sourcefile_ts_4.MediaType.Unknown;
      }
    }
    function processLocalImports(
      sources,
      specifiers,
      referrer,
      processJsImports = false
    ) {
      if (!specifiers.length) {
        return [];
      }
      const moduleNames = specifiers.map(
        referrer
          ? ([, specifier]) => resolveSpecifier(specifier, referrer)
          : ([, specifier]) => specifier
      );
      for (let i = 0; i < moduleNames.length; i++) {
        const moduleName = moduleNames[i];
        util_ts_17.assert(
          moduleName in sources,
          `Missing module in sources: "${moduleName}"`
        );
        const sourceFile =
          sourcefile_ts_4.SourceFile.get(moduleName) ||
          new sourcefile_ts_4.SourceFile({
            url: moduleName,
            filename: moduleName,
            sourceCode: sources[moduleName],
            mediaType: getMediaType(moduleName),
          });
        sourceFile.cache(specifiers[i][0], referrer);
        if (!sourceFile.processed) {
          processLocalImports(
            sources,
            sourceFile.imports(processJsImports),
            sourceFile.url,
            processJsImports
          );
        }
      }
      return moduleNames;
    }
    exports_79("processLocalImports", processLocalImports);
    async function processImports(
      specifiers,
      referrer,
      processJsImports = false
    ) {
      if (!specifiers.length) {
        return [];
      }
      const sources = specifiers.map(([, moduleSpecifier]) => moduleSpecifier);
      const resolvedSources = resolveModules(sources, referrer);
      const sourceFiles = await fetchSourceFiles(resolvedSources, referrer);
      util_ts_17.assert(sourceFiles.length === specifiers.length);
      for (let i = 0; i < sourceFiles.length; i++) {
        const sourceFileJson = sourceFiles[i];
        const sourceFile =
          sourcefile_ts_4.SourceFile.get(sourceFileJson.url) ||
          new sourcefile_ts_4.SourceFile(sourceFileJson);
        sourceFile.cache(specifiers[i][0], referrer);
        if (!sourceFile.processed) {
          await processImports(
            sourceFile.imports(processJsImports),
            sourceFile.url,
            processJsImports
          );
        }
      }
      return resolvedSources;
    }
    exports_79("processImports", processImports);
    return {
      setters: [
        function (sourcefile_ts_4_1) {
          sourcefile_ts_4 = sourcefile_ts_4_1;
        },
        function (util_ts_16_1) {
          util_ts_16 = util_ts_16_1;
        },
        function (dir_ts_3_1) {
          dir_ts_3 = dir_ts_3_1;
        },
        function (util_ts_17_1) {
          util_ts_17 = util_ts_17_1;
          util = util_ts_17_1;
        },
        function (compilerOps_2) {
          compilerOps = compilerOps_2;
        },
      ],
      execute: function () {},
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/diagnostics_util.ts",
  ["$deno$/diagnostics.ts"],
  function (exports_80, context_80) {
    "use strict";
    let diagnostics_ts_2;
    const __moduleName = context_80 && context_80.id;
    function fromDiagnosticCategory(category) {
      switch (category) {
        case ts.DiagnosticCategory.Error:
          return diagnostics_ts_2.DiagnosticCategory.Error;
        case ts.DiagnosticCategory.Message:
          return diagnostics_ts_2.DiagnosticCategory.Info;
        case ts.DiagnosticCategory.Suggestion:
          return diagnostics_ts_2.DiagnosticCategory.Suggestion;
        case ts.DiagnosticCategory.Warning:
          return diagnostics_ts_2.DiagnosticCategory.Warning;
        default:
          throw new Error(
            `Unexpected DiagnosticCategory: "${category}"/"${ts.DiagnosticCategory[category]}"`
          );
      }
    }
    function getSourceInformation(sourceFile, start, length) {
      const scriptResourceName = sourceFile.fileName;
      const {
        line: lineNumber,
        character: startColumn,
      } = sourceFile.getLineAndCharacterOfPosition(start);
      const endPosition = sourceFile.getLineAndCharacterOfPosition(
        start + length
      );
      const endColumn =
        lineNumber === endPosition.line ? endPosition.character : startColumn;
      const lastLineInFile = sourceFile.getLineAndCharacterOfPosition(
        sourceFile.text.length
      ).line;
      const lineStart = sourceFile.getPositionOfLineAndCharacter(lineNumber, 0);
      const lineEnd =
        lineNumber < lastLineInFile
          ? sourceFile.getPositionOfLineAndCharacter(lineNumber + 1, 0)
          : sourceFile.text.length;
      const sourceLine = sourceFile.text
        .slice(lineStart, lineEnd)
        .replace(/\s+$/g, "")
        .replace("\t", " ");
      return {
        sourceLine,
        lineNumber,
        scriptResourceName,
        startColumn,
        endColumn,
      };
    }
    function fromDiagnosticMessageChain(messageChain) {
      if (!messageChain) {
        return undefined;
      }
      return messageChain.map(
        ({ messageText: message, code, category, next }) => {
          return {
            message,
            code,
            category: fromDiagnosticCategory(category),
            next: fromDiagnosticMessageChain(next),
          };
        }
      );
    }
    function parseDiagnostic(item) {
      const {
        messageText,
        category: sourceCategory,
        code,
        file,
        start: startPosition,
        length,
      } = item;
      const sourceInfo =
        file && startPosition && length
          ? getSourceInformation(file, startPosition, length)
          : undefined;
      const endPosition =
        startPosition && length ? startPosition + length : undefined;
      const category = fromDiagnosticCategory(sourceCategory);
      let message;
      let messageChain;
      if (typeof messageText === "string") {
        message = messageText;
      } else {
        message = messageText.messageText;
        messageChain = fromDiagnosticMessageChain([messageText])[0];
      }
      const base = {
        message,
        messageChain,
        code,
        category,
        startPosition,
        endPosition,
      };
      return sourceInfo ? { ...base, ...sourceInfo } : base;
    }
    function parseRelatedInformation(relatedInformation) {
      const result = [];
      for (const item of relatedInformation) {
        result.push(parseDiagnostic(item));
      }
      return result;
    }
    function fromTypeScriptDiagnostic(diagnostics) {
      const items = [];
      for (const sourceDiagnostic of diagnostics) {
        const item = parseDiagnostic(sourceDiagnostic);
        if (sourceDiagnostic.relatedInformation) {
          item.relatedInformation = parseRelatedInformation(
            sourceDiagnostic.relatedInformation
          );
        }
        items.push(item);
      }
      return { items };
    }
    exports_80("fromTypeScriptDiagnostic", fromTypeScriptDiagnostic);
    return {
      setters: [
        function (diagnostics_ts_2_1) {
          diagnostics_ts_2 = diagnostics_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register("$deno$/web/streams/shared-internals.ts", [], function (
  exports_81,
  context_81
) {
  "use strict";
  let objectCloneMemo, sharedArrayBufferSupported_;
  const __moduleName = context_81 && context_81.id;
  function isInteger(value) {
    if (!isFinite(value)) {
      // covers NaN, +Infinity and -Infinity
      return false;
    }
    const absValue = Math.abs(value);
    return Math.floor(absValue) === absValue;
  }
  exports_81("isInteger", isInteger);
  function isFiniteNonNegativeNumber(value) {
    if (!(typeof value === "number" && isFinite(value))) {
      // covers NaN, +Infinity and -Infinity
      return false;
    }
    return value >= 0;
  }
  exports_81("isFiniteNonNegativeNumber", isFiniteNonNegativeNumber);
  function isAbortSignal(signal) {
    if (typeof signal !== "object" || signal === null) {
      return false;
    }
    try {
      // TODO
      // calling signal.aborted() probably isn't the right way to perform this test
      // https://github.com/stardazed/sd-streams/blob/master/packages/streams/src/shared-internals.ts#L41
      signal.aborted();
      return true;
    } catch (err) {
      return false;
    }
  }
  exports_81("isAbortSignal", isAbortSignal);
  function invokeOrNoop(o, p, args) {
    // Assert: O is not undefined.
    // Assert: IsPropertyKey(P) is true.
    // Assert: args is a List.
    const method = o[p]; // tslint:disable-line:ban-types
    if (method === undefined) {
      return undefined;
    }
    return Function.prototype.apply.call(method, o, args);
  }
  exports_81("invokeOrNoop", invokeOrNoop);
  function cloneArrayBuffer(
    srcBuffer,
    srcByteOffset,
    srcLength,
    _cloneConstructor
  ) {
    // this function fudges the return type but SharedArrayBuffer is disabled for a while anyway
    return srcBuffer.slice(srcByteOffset, srcByteOffset + srcLength);
  }
  exports_81("cloneArrayBuffer", cloneArrayBuffer);
  function transferArrayBuffer(buffer) {
    // This would in a JS engine context detach the buffer's backing store and return
    // a new ArrayBuffer with the same backing store, invalidating `buffer`,
    // i.e. a move operation in C++ parlance.
    // Sadly ArrayBuffer.transfer is yet to be implemented by a single browser vendor.
    return buffer.slice(0); // copies instead of moves
  }
  exports_81("transferArrayBuffer", transferArrayBuffer);
  function copyDataBlockBytes(toBlock, toIndex, fromBlock, fromIndex, count) {
    new Uint8Array(toBlock, toIndex, count).set(
      new Uint8Array(fromBlock, fromIndex, count)
    );
  }
  exports_81("copyDataBlockBytes", copyDataBlockBytes);
  function supportsSharedArrayBuffer() {
    if (sharedArrayBufferSupported_ === undefined) {
      try {
        new SharedArrayBuffer(16);
        sharedArrayBufferSupported_ = true;
      } catch (e) {
        sharedArrayBufferSupported_ = false;
      }
    }
    return sharedArrayBufferSupported_;
  }
  function cloneValue(value) {
    const valueType = typeof value;
    switch (valueType) {
      case "number":
      case "string":
      case "boolean":
      case "undefined":
      // @ts-ignore
      case "bigint":
        return value;
      case "object": {
        if (objectCloneMemo.has(value)) {
          return objectCloneMemo.get(value);
        }
        if (value === null) {
          return value;
        }
        if (value instanceof Date) {
          return new Date(value.valueOf());
        }
        if (value instanceof RegExp) {
          return new RegExp(value);
        }
        if (supportsSharedArrayBuffer() && value instanceof SharedArrayBuffer) {
          return value;
        }
        if (value instanceof ArrayBuffer) {
          const cloned = cloneArrayBuffer(
            value,
            0,
            value.byteLength,
            ArrayBuffer
          );
          objectCloneMemo.set(value, cloned);
          return cloned;
        }
        if (ArrayBuffer.isView(value)) {
          const clonedBuffer = cloneValue(value.buffer);
          // Use DataViewConstructor type purely for type-checking, can be a DataView or TypedArray.
          // They use the same constructor signature, only DataView has a length in bytes and TypedArrays
          // use a length in terms of elements, so we adjust for that.
          let length;
          if (value instanceof DataView) {
            length = value.byteLength;
          } else {
            length = value.length;
          }
          return new value.constructor(clonedBuffer, value.byteOffset, length);
        }
        if (value instanceof Map) {
          const clonedMap = new Map();
          objectCloneMemo.set(value, clonedMap);
          value.forEach((v, k) => clonedMap.set(k, cloneValue(v)));
          return clonedMap;
        }
        if (value instanceof Set) {
          const clonedSet = new Map();
          objectCloneMemo.set(value, clonedSet);
          value.forEach((v, k) => clonedSet.set(k, cloneValue(v)));
          return clonedSet;
        }
        // generic object
        const clonedObj = {};
        objectCloneMemo.set(value, clonedObj);
        const sourceKeys = Object.getOwnPropertyNames(value);
        for (const key of sourceKeys) {
          clonedObj[key] = cloneValue(value[key]);
        }
        return clonedObj;
      }
      case "symbol":
      case "function":
      default:
        // TODO this should be a DOMException,
        // https://github.com/stardazed/sd-streams/blob/master/packages/streams/src/shared-internals.ts#L171
        throw new Error("Uncloneable value in stream");
    }
  }
  exports_81("cloneValue", cloneValue);
  function promiseCall(f, v, args) {
    // tslint:disable-line:ban-types
    try {
      const result = Function.prototype.apply.call(f, v, args);
      return Promise.resolve(result);
    } catch (err) {
      return Promise.reject(err);
    }
  }
  exports_81("promiseCall", promiseCall);
  function createAlgorithmFromUnderlyingMethod(obj, methodName, extraArgs) {
    const method = obj[methodName];
    if (method === undefined) {
      return () => Promise.resolve(undefined);
    }
    if (typeof method !== "function") {
      throw new TypeError(`Field "${methodName}" is not a function.`);
    }
    return function (...fnArgs) {
      return promiseCall(method, obj, fnArgs.concat(extraArgs));
    };
  }
  exports_81(
    "createAlgorithmFromUnderlyingMethod",
    createAlgorithmFromUnderlyingMethod
  );
  /*
    Deprecated for now, all usages replaced by readableStreamCreateReadResult
    
    function createIterResultObject<T>(value: T, done: boolean): IteratorResult<T> {
        return { value, done };
    }
    */
  function validateAndNormalizeHighWaterMark(hwm) {
    const highWaterMark = Number(hwm);
    if (isNaN(highWaterMark) || highWaterMark < 0) {
      throw new RangeError(
        "highWaterMark must be a valid, non-negative integer."
      );
    }
    return highWaterMark;
  }
  exports_81(
    "validateAndNormalizeHighWaterMark",
    validateAndNormalizeHighWaterMark
  );
  function makeSizeAlgorithmFromSizeFunction(sizeFn) {
    if (typeof sizeFn !== "function" && typeof sizeFn !== "undefined") {
      throw new TypeError("size function must be undefined or a function");
    }
    return function (chunk) {
      if (typeof sizeFn === "function") {
        return sizeFn(chunk);
      }
      return 1;
    };
  }
  exports_81(
    "makeSizeAlgorithmFromSizeFunction",
    makeSizeAlgorithmFromSizeFunction
  );
  function createControlledPromise() {
    const conProm = {
      state: 0 /* Pending */,
    };
    conProm.promise = new Promise(function (resolve, reject) {
      conProm.resolve = function (v) {
        conProm.state = 1 /* Resolved */;
        resolve(v);
      };
      conProm.reject = function (e) {
        conProm.state = 2 /* Rejected */;
        reject(e);
      };
    });
    return conProm;
  }
  exports_81("createControlledPromise", createControlledPromise);
  return {
    setters: [],
    execute: function () {
      // common stream fields
      exports_81("state_", Symbol("state_"));
      exports_81("storedError_", Symbol("storedError_"));
      // helper memoisation map for object values
      // weak so it doesn't keep memoized versions of old objects indefinitely.
      objectCloneMemo = new WeakMap();
    },
  };
});
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register("$deno$/web/streams/queue.ts", [], function (
  exports_82,
  context_82
) {
  "use strict";
  let CHUNK_SIZE, QueueImpl;
  const __moduleName = context_82 && context_82.id;
  return {
    setters: [],
    execute: function () {
      CHUNK_SIZE = 16384;
      QueueImpl = class QueueImpl {
        constructor() {
          this.chunks_ = [[]];
          this.readChunk_ = this.writeChunk_ = this.chunks_[0];
          this.length_ = 0;
        }
        push(t) {
          this.writeChunk_.push(t);
          this.length_ += 1;
          if (this.writeChunk_.length === CHUNK_SIZE) {
            this.writeChunk_ = [];
            this.chunks_.push(this.writeChunk_);
          }
        }
        front() {
          if (this.length_ === 0) {
            return undefined;
          }
          return this.readChunk_[0];
        }
        shift() {
          if (this.length_ === 0) {
            return undefined;
          }
          const t = this.readChunk_.shift();
          this.length_ -= 1;
          if (
            this.readChunk_.length === 0 &&
            this.readChunk_ !== this.writeChunk_
          ) {
            this.chunks_.shift();
            this.readChunk_ = this.chunks_[0];
          }
          return t;
        }
        get length() {
          return this.length_;
        }
      };
      exports_82("QueueImpl", QueueImpl);
    },
  };
});
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/queue-mixin.ts",
  ["$deno$/web/streams/queue.ts", "$deno$/web/streams/shared-internals.ts"],
  function (exports_83, context_83) {
    "use strict";
    let queue_ts_1, shared_internals_ts_1, queue_, queueTotalSize_;
    const __moduleName = context_83 && context_83.id;
    function dequeueValue(container) {
      // Assert: container has[[queue]] and[[queueTotalSize]] internal slots.
      // Assert: container.[[queue]] is not empty.
      const pair = container[queue_].shift();
      const newTotalSize = container[queueTotalSize_] - pair.size;
      container[queueTotalSize_] = Math.max(0, newTotalSize); // < 0 can occur due to rounding errors.
      return pair.value;
    }
    exports_83("dequeueValue", dequeueValue);
    function enqueueValueWithSize(container, value, size) {
      // Assert: container has[[queue]] and[[queueTotalSize]] internal slots.
      if (!shared_internals_ts_1.isFiniteNonNegativeNumber(size)) {
        throw new RangeError(
          "Chunk size must be a non-negative, finite numbers"
        );
      }
      container[queue_].push({ value, size });
      container[queueTotalSize_] += size;
    }
    exports_83("enqueueValueWithSize", enqueueValueWithSize);
    function peekQueueValue(container) {
      // Assert: container has[[queue]] and[[queueTotalSize]] internal slots.
      // Assert: container.[[queue]] is not empty.
      return container[queue_].front().value;
    }
    exports_83("peekQueueValue", peekQueueValue);
    function resetQueue(container) {
      // Chrome (as of v67) has a steep performance cliff with large arrays
      // and shift(), around about 50k elements. While this is an unusual case
      // we use a simple wrapper around shift and push that is chunked to
      // avoid this pitfall.
      // @see: https://github.com/stardazed/sd-streams/issues/1
      container[queue_] = new queue_ts_1.QueueImpl();
      // The code below can be used as a plain array implementation of the
      // Queue interface.
      // const q = [] as any;
      // q.front = function() { return this[0]; };
      // container[queue_] = q;
      container[queueTotalSize_] = 0;
    }
    exports_83("resetQueue", resetQueue);
    return {
      setters: [
        function (queue_ts_1_1) {
          queue_ts_1 = queue_ts_1_1;
        },
        function (shared_internals_ts_1_1) {
          shared_internals_ts_1 = shared_internals_ts_1_1;
        },
      ],
      execute: function () {
        exports_83("queue_", (queue_ = Symbol("queue_")));
        exports_83(
          "queueTotalSize_",
          (queueTotalSize_ = Symbol("queueTotalSize_"))
        );
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-internals.ts",
  [
    "$deno$/web/streams/shared-internals.ts",
    "$deno$/web/streams/queue-mixin.ts",
  ],
  function (exports_84, context_84) {
    "use strict";
    let shared,
      q,
      controlledReadableStream_,
      pullAlgorithm_,
      cancelAlgorithm_,
      strategySizeAlgorithm_,
      strategyHWM_,
      started_,
      closeRequested_,
      pullAgain_,
      pulling_,
      cancelSteps_,
      pullSteps_,
      autoAllocateChunkSize_,
      byobRequest_,
      controlledReadableByteStream_,
      pendingPullIntos_,
      closedPromise_,
      ownerReadableStream_,
      readRequests_,
      readIntoRequests_,
      associatedReadableByteStreamController_,
      view_,
      reader_,
      readableStreamController_;
    const __moduleName = context_84 && context_84.id;
    // ---- Stream
    function initializeReadableStream(stream) {
      stream[shared.state_] = "readable";
      stream[reader_] = undefined;
      stream[shared.storedError_] = undefined;
      stream[readableStreamController_] = undefined; // mark slot as used for brand check
    }
    exports_84("initializeReadableStream", initializeReadableStream);
    function isReadableStream(value) {
      if (typeof value !== "object" || value === null) {
        return false;
      }
      return readableStreamController_ in value;
    }
    exports_84("isReadableStream", isReadableStream);
    function isReadableStreamLocked(stream) {
      return stream[reader_] !== undefined;
    }
    exports_84("isReadableStreamLocked", isReadableStreamLocked);
    function readableStreamGetNumReadIntoRequests(stream) {
      // TODO remove the "as unknown" cast
      // This is in to workaround a compiler error
      // error TS2352: Conversion of type 'SDReadableStreamReader<OutputType>' to type 'SDReadableStreamBYOBReader' may be a mistake because neither type sufficiently overlaps with the other. If this was intentional, convert the expression to 'unknown' first.
      // Type 'SDReadableStreamReader<OutputType>' is missing the following properties from type 'SDReadableStreamBYOBReader': read, [readIntoRequests_]
      const reader = stream[reader_];
      if (reader === undefined) {
        return 0;
      }
      return reader[readIntoRequests_].length;
    }
    exports_84(
      "readableStreamGetNumReadIntoRequests",
      readableStreamGetNumReadIntoRequests
    );
    function readableStreamGetNumReadRequests(stream) {
      const reader = stream[reader_];
      if (reader === undefined) {
        return 0;
      }
      return reader[readRequests_].length;
    }
    exports_84(
      "readableStreamGetNumReadRequests",
      readableStreamGetNumReadRequests
    );
    function readableStreamCreateReadResult(value, done, forAuthorCode) {
      const prototype = forAuthorCode ? Object.prototype : null;
      const result = Object.create(prototype);
      result.value = value;
      result.done = done;
      return result;
    }
    exports_84(
      "readableStreamCreateReadResult",
      readableStreamCreateReadResult
    );
    function readableStreamAddReadIntoRequest(stream, forAuthorCode) {
      // Assert: ! IsReadableStreamBYOBReader(stream.[[reader]]) is true.
      // Assert: stream.[[state]] is "readable" or "closed".
      const reader = stream[reader_];
      const conProm = shared.createControlledPromise();
      conProm.forAuthorCode = forAuthorCode;
      reader[readIntoRequests_].push(conProm);
      return conProm.promise;
    }
    exports_84(
      "readableStreamAddReadIntoRequest",
      readableStreamAddReadIntoRequest
    );
    function readableStreamAddReadRequest(stream, forAuthorCode) {
      // Assert: ! IsReadableStreamDefaultReader(stream.[[reader]]) is true.
      // Assert: stream.[[state]] is "readable".
      const reader = stream[reader_];
      const conProm = shared.createControlledPromise();
      conProm.forAuthorCode = forAuthorCode;
      reader[readRequests_].push(conProm);
      return conProm.promise;
    }
    exports_84("readableStreamAddReadRequest", readableStreamAddReadRequest);
    function readableStreamHasBYOBReader(stream) {
      const reader = stream[reader_];
      return isReadableStreamBYOBReader(reader);
    }
    exports_84("readableStreamHasBYOBReader", readableStreamHasBYOBReader);
    function readableStreamHasDefaultReader(stream) {
      const reader = stream[reader_];
      return isReadableStreamDefaultReader(reader);
    }
    exports_84(
      "readableStreamHasDefaultReader",
      readableStreamHasDefaultReader
    );
    function readableStreamCancel(stream, reason) {
      if (stream[shared.state_] === "closed") {
        return Promise.resolve(undefined);
      }
      if (stream[shared.state_] === "errored") {
        return Promise.reject(stream[shared.storedError_]);
      }
      readableStreamClose(stream);
      const sourceCancelPromise = stream[readableStreamController_][
        cancelSteps_
      ](reason);
      return sourceCancelPromise.then((_) => undefined);
    }
    exports_84("readableStreamCancel", readableStreamCancel);
    function readableStreamClose(stream) {
      // Assert: stream.[[state]] is "readable".
      stream[shared.state_] = "closed";
      const reader = stream[reader_];
      if (reader === undefined) {
        return;
      }
      if (isReadableStreamDefaultReader(reader)) {
        for (const readRequest of reader[readRequests_]) {
          readRequest.resolve(
            readableStreamCreateReadResult(
              undefined,
              true,
              readRequest.forAuthorCode
            )
          );
        }
        reader[readRequests_] = [];
      }
      reader[closedPromise_].resolve();
      reader[closedPromise_].promise.catch(() => {});
    }
    exports_84("readableStreamClose", readableStreamClose);
    function readableStreamError(stream, error) {
      if (stream[shared.state_] !== "readable") {
        throw new RangeError("Stream is in an invalid state");
      }
      stream[shared.state_] = "errored";
      stream[shared.storedError_] = error;
      const reader = stream[reader_];
      if (reader === undefined) {
        return;
      }
      if (isReadableStreamDefaultReader(reader)) {
        for (const readRequest of reader[readRequests_]) {
          readRequest.reject(error);
        }
        reader[readRequests_] = [];
      } else {
        // Assert: IsReadableStreamBYOBReader(reader).
        // TODO remove the "as unknown" cast
        const readIntoRequests = reader[readIntoRequests_];
        for (const readIntoRequest of readIntoRequests) {
          readIntoRequest.reject(error);
        }
        // TODO remove the "as unknown" cast
        reader[readIntoRequests_] = [];
      }
      reader[closedPromise_].reject(error);
    }
    exports_84("readableStreamError", readableStreamError);
    // ---- Readers
    function isReadableStreamDefaultReader(reader) {
      if (typeof reader !== "object" || reader === null) {
        return false;
      }
      return readRequests_ in reader;
    }
    exports_84("isReadableStreamDefaultReader", isReadableStreamDefaultReader);
    function isReadableStreamBYOBReader(reader) {
      if (typeof reader !== "object" || reader === null) {
        return false;
      }
      return readIntoRequests_ in reader;
    }
    exports_84("isReadableStreamBYOBReader", isReadableStreamBYOBReader);
    function readableStreamReaderGenericInitialize(reader, stream) {
      reader[ownerReadableStream_] = stream;
      stream[reader_] = reader;
      const streamState = stream[shared.state_];
      reader[closedPromise_] = shared.createControlledPromise();
      if (streamState === "readable") {
        // leave as is
      } else if (streamState === "closed") {
        reader[closedPromise_].resolve(undefined);
      } else {
        reader[closedPromise_].reject(stream[shared.storedError_]);
        reader[closedPromise_].promise.catch(() => {});
      }
    }
    exports_84(
      "readableStreamReaderGenericInitialize",
      readableStreamReaderGenericInitialize
    );
    function readableStreamReaderGenericRelease(reader) {
      // Assert: reader.[[ownerReadableStream]] is not undefined.
      // Assert: reader.[[ownerReadableStream]].[[reader]] is reader.
      const stream = reader[ownerReadableStream_];
      if (stream === undefined) {
        throw new TypeError("Reader is in an inconsistent state");
      }
      if (stream[shared.state_] === "readable") {
        // code moved out
      } else {
        reader[closedPromise_] = shared.createControlledPromise();
      }
      reader[closedPromise_].reject(new TypeError());
      reader[closedPromise_].promise.catch(() => {});
      stream[reader_] = undefined;
      reader[ownerReadableStream_] = undefined;
    }
    exports_84(
      "readableStreamReaderGenericRelease",
      readableStreamReaderGenericRelease
    );
    function readableStreamBYOBReaderRead(reader, view, forAuthorCode = false) {
      const stream = reader[ownerReadableStream_];
      // Assert: stream is not undefined.
      if (stream[shared.state_] === "errored") {
        return Promise.reject(stream[shared.storedError_]);
      }
      return readableByteStreamControllerPullInto(
        stream[readableStreamController_],
        view,
        forAuthorCode
      );
    }
    exports_84("readableStreamBYOBReaderRead", readableStreamBYOBReaderRead);
    function readableStreamDefaultReaderRead(reader, forAuthorCode = false) {
      const stream = reader[ownerReadableStream_];
      // Assert: stream is not undefined.
      if (stream[shared.state_] === "closed") {
        return Promise.resolve(
          readableStreamCreateReadResult(undefined, true, forAuthorCode)
        );
      }
      if (stream[shared.state_] === "errored") {
        return Promise.reject(stream[shared.storedError_]);
      }
      // Assert: stream.[[state]] is "readable".
      return stream[readableStreamController_][pullSteps_](forAuthorCode);
    }
    exports_84(
      "readableStreamDefaultReaderRead",
      readableStreamDefaultReaderRead
    );
    function readableStreamFulfillReadIntoRequest(stream, chunk, done) {
      // TODO remove the "as unknown" cast
      const reader = stream[reader_];
      const readIntoRequest = reader[readIntoRequests_].shift(); // <-- length check done in caller
      readIntoRequest.resolve(
        readableStreamCreateReadResult(
          chunk,
          done,
          readIntoRequest.forAuthorCode
        )
      );
    }
    exports_84(
      "readableStreamFulfillReadIntoRequest",
      readableStreamFulfillReadIntoRequest
    );
    function readableStreamFulfillReadRequest(stream, chunk, done) {
      const reader = stream[reader_];
      const readRequest = reader[readRequests_].shift(); // <-- length check done in caller
      readRequest.resolve(
        readableStreamCreateReadResult(chunk, done, readRequest.forAuthorCode)
      );
    }
    exports_84(
      "readableStreamFulfillReadRequest",
      readableStreamFulfillReadRequest
    );
    // ---- DefaultController
    function setUpReadableStreamDefaultController(
      stream,
      controller,
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      sizeAlgorithm
    ) {
      // Assert: stream.[[readableStreamController]] is undefined.
      controller[controlledReadableStream_] = stream;
      q.resetQueue(controller);
      controller[started_] = false;
      controller[closeRequested_] = false;
      controller[pullAgain_] = false;
      controller[pulling_] = false;
      controller[strategySizeAlgorithm_] = sizeAlgorithm;
      controller[strategyHWM_] = highWaterMark;
      controller[pullAlgorithm_] = pullAlgorithm;
      controller[cancelAlgorithm_] = cancelAlgorithm;
      stream[readableStreamController_] = controller;
      const startResult = startAlgorithm();
      Promise.resolve(startResult).then(
        (_) => {
          controller[started_] = true;
          // Assert: controller.[[pulling]] is false.
          // Assert: controller.[[pullAgain]] is false.
          readableStreamDefaultControllerCallPullIfNeeded(controller);
        },
        (error) => {
          readableStreamDefaultControllerError(controller, error);
        }
      );
    }
    exports_84(
      "setUpReadableStreamDefaultController",
      setUpReadableStreamDefaultController
    );
    function isReadableStreamDefaultController(value) {
      if (typeof value !== "object" || value === null) {
        return false;
      }
      return controlledReadableStream_ in value;
    }
    exports_84(
      "isReadableStreamDefaultController",
      isReadableStreamDefaultController
    );
    function readableStreamDefaultControllerHasBackpressure(controller) {
      return !readableStreamDefaultControllerShouldCallPull(controller);
    }
    exports_84(
      "readableStreamDefaultControllerHasBackpressure",
      readableStreamDefaultControllerHasBackpressure
    );
    function readableStreamDefaultControllerCanCloseOrEnqueue(controller) {
      const state = controller[controlledReadableStream_][shared.state_];
      return controller[closeRequested_] === false && state === "readable";
    }
    exports_84(
      "readableStreamDefaultControllerCanCloseOrEnqueue",
      readableStreamDefaultControllerCanCloseOrEnqueue
    );
    function readableStreamDefaultControllerGetDesiredSize(controller) {
      const state = controller[controlledReadableStream_][shared.state_];
      if (state === "errored") {
        return null;
      }
      if (state === "closed") {
        return 0;
      }
      return controller[strategyHWM_] - controller[q.queueTotalSize_];
    }
    exports_84(
      "readableStreamDefaultControllerGetDesiredSize",
      readableStreamDefaultControllerGetDesiredSize
    );
    function readableStreamDefaultControllerClose(controller) {
      // Assert: !ReadableStreamDefaultControllerCanCloseOrEnqueue(controller) is true.
      controller[closeRequested_] = true;
      const stream = controller[controlledReadableStream_];
      if (controller[q.queue_].length === 0) {
        readableStreamDefaultControllerClearAlgorithms(controller);
        readableStreamClose(stream);
      }
    }
    exports_84(
      "readableStreamDefaultControllerClose",
      readableStreamDefaultControllerClose
    );
    function readableStreamDefaultControllerEnqueue(controller, chunk) {
      const stream = controller[controlledReadableStream_];
      // Assert: !ReadableStreamDefaultControllerCanCloseOrEnqueue(controller) is true.
      if (
        isReadableStreamLocked(stream) &&
        readableStreamGetNumReadRequests(stream) > 0
      ) {
        readableStreamFulfillReadRequest(stream, chunk, false);
      } else {
        // Let result be the result of performing controller.[[strategySizeAlgorithm]], passing in chunk,
        // and interpreting the result as an ECMAScript completion value.
        // impl note: assuming that in JS land this just means try/catch with rethrow
        let chunkSize;
        try {
          chunkSize = controller[strategySizeAlgorithm_](chunk);
        } catch (error) {
          readableStreamDefaultControllerError(controller, error);
          throw error;
        }
        try {
          q.enqueueValueWithSize(controller, chunk, chunkSize);
        } catch (error) {
          readableStreamDefaultControllerError(controller, error);
          throw error;
        }
      }
      readableStreamDefaultControllerCallPullIfNeeded(controller);
    }
    exports_84(
      "readableStreamDefaultControllerEnqueue",
      readableStreamDefaultControllerEnqueue
    );
    function readableStreamDefaultControllerError(controller, error) {
      const stream = controller[controlledReadableStream_];
      if (stream[shared.state_] !== "readable") {
        return;
      }
      q.resetQueue(controller);
      readableStreamDefaultControllerClearAlgorithms(controller);
      readableStreamError(stream, error);
    }
    exports_84(
      "readableStreamDefaultControllerError",
      readableStreamDefaultControllerError
    );
    function readableStreamDefaultControllerCallPullIfNeeded(controller) {
      if (!readableStreamDefaultControllerShouldCallPull(controller)) {
        return;
      }
      if (controller[pulling_]) {
        controller[pullAgain_] = true;
        return;
      }
      if (controller[pullAgain_]) {
        throw new RangeError("Stream controller is in an invalid state.");
      }
      controller[pulling_] = true;
      controller[pullAlgorithm_](controller).then(
        (_) => {
          controller[pulling_] = false;
          if (controller[pullAgain_]) {
            controller[pullAgain_] = false;
            readableStreamDefaultControllerCallPullIfNeeded(controller);
          }
        },
        (error) => {
          readableStreamDefaultControllerError(controller, error);
        }
      );
    }
    exports_84(
      "readableStreamDefaultControllerCallPullIfNeeded",
      readableStreamDefaultControllerCallPullIfNeeded
    );
    function readableStreamDefaultControllerShouldCallPull(controller) {
      const stream = controller[controlledReadableStream_];
      if (!readableStreamDefaultControllerCanCloseOrEnqueue(controller)) {
        return false;
      }
      if (controller[started_] === false) {
        return false;
      }
      if (
        isReadableStreamLocked(stream) &&
        readableStreamGetNumReadRequests(stream) > 0
      ) {
        return true;
      }
      const desiredSize = readableStreamDefaultControllerGetDesiredSize(
        controller
      );
      if (desiredSize === null) {
        throw new RangeError("Stream is in an invalid state.");
      }
      return desiredSize > 0;
    }
    exports_84(
      "readableStreamDefaultControllerShouldCallPull",
      readableStreamDefaultControllerShouldCallPull
    );
    function readableStreamDefaultControllerClearAlgorithms(controller) {
      controller[pullAlgorithm_] = undefined;
      controller[cancelAlgorithm_] = undefined;
      controller[strategySizeAlgorithm_] = undefined;
    }
    exports_84(
      "readableStreamDefaultControllerClearAlgorithms",
      readableStreamDefaultControllerClearAlgorithms
    );
    // ---- BYOBController
    function setUpReadableByteStreamController(
      stream,
      controller,
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      autoAllocateChunkSize
    ) {
      // Assert: stream.[[readableStreamController]] is undefined.
      if (stream[readableStreamController_] !== undefined) {
        throw new TypeError("Cannot reuse streams");
      }
      if (autoAllocateChunkSize !== undefined) {
        if (
          !shared.isInteger(autoAllocateChunkSize) ||
          autoAllocateChunkSize <= 0
        ) {
          throw new RangeError(
            "autoAllocateChunkSize must be a positive, finite integer"
          );
        }
      }
      // Set controller.[[controlledReadableByteStream]] to stream.
      controller[controlledReadableByteStream_] = stream;
      // Set controller.[[pullAgain]] and controller.[[pulling]] to false.
      controller[pullAgain_] = false;
      controller[pulling_] = false;
      readableByteStreamControllerClearPendingPullIntos(controller);
      q.resetQueue(controller);
      controller[closeRequested_] = false;
      controller[started_] = false;
      controller[strategyHWM_] = shared.validateAndNormalizeHighWaterMark(
        highWaterMark
      );
      controller[pullAlgorithm_] = pullAlgorithm;
      controller[cancelAlgorithm_] = cancelAlgorithm;
      controller[autoAllocateChunkSize_] = autoAllocateChunkSize;
      controller[pendingPullIntos_] = [];
      stream[readableStreamController_] = controller;
      // Let startResult be the result of performing startAlgorithm.
      const startResult = startAlgorithm();
      Promise.resolve(startResult).then(
        (_) => {
          controller[started_] = true;
          // Assert: controller.[[pulling]] is false.
          // Assert: controller.[[pullAgain]] is false.
          readableByteStreamControllerCallPullIfNeeded(controller);
        },
        (error) => {
          readableByteStreamControllerError(controller, error);
        }
      );
    }
    exports_84(
      "setUpReadableByteStreamController",
      setUpReadableByteStreamController
    );
    function isReadableStreamBYOBRequest(value) {
      if (typeof value !== "object" || value === null) {
        return false;
      }
      return associatedReadableByteStreamController_ in value;
    }
    exports_84("isReadableStreamBYOBRequest", isReadableStreamBYOBRequest);
    function isReadableByteStreamController(value) {
      if (typeof value !== "object" || value === null) {
        return false;
      }
      return controlledReadableByteStream_ in value;
    }
    exports_84(
      "isReadableByteStreamController",
      isReadableByteStreamController
    );
    function readableByteStreamControllerCallPullIfNeeded(controller) {
      if (!readableByteStreamControllerShouldCallPull(controller)) {
        return;
      }
      if (controller[pulling_]) {
        controller[pullAgain_] = true;
        return;
      }
      // Assert: controller.[[pullAgain]] is false.
      controller[pulling_] = true;
      controller[pullAlgorithm_](controller).then(
        (_) => {
          controller[pulling_] = false;
          if (controller[pullAgain_]) {
            controller[pullAgain_] = false;
            readableByteStreamControllerCallPullIfNeeded(controller);
          }
        },
        (error) => {
          readableByteStreamControllerError(controller, error);
        }
      );
    }
    exports_84(
      "readableByteStreamControllerCallPullIfNeeded",
      readableByteStreamControllerCallPullIfNeeded
    );
    function readableByteStreamControllerClearAlgorithms(controller) {
      controller[pullAlgorithm_] = undefined;
      controller[cancelAlgorithm_] = undefined;
    }
    exports_84(
      "readableByteStreamControllerClearAlgorithms",
      readableByteStreamControllerClearAlgorithms
    );
    function readableByteStreamControllerClearPendingPullIntos(controller) {
      readableByteStreamControllerInvalidateBYOBRequest(controller);
      controller[pendingPullIntos_] = [];
    }
    exports_84(
      "readableByteStreamControllerClearPendingPullIntos",
      readableByteStreamControllerClearPendingPullIntos
    );
    function readableByteStreamControllerClose(controller) {
      const stream = controller[controlledReadableByteStream_];
      // Assert: controller.[[closeRequested]] is false.
      // Assert: stream.[[state]] is "readable".
      if (controller[q.queueTotalSize_] > 0) {
        controller[closeRequested_] = true;
        return;
      }
      if (controller[pendingPullIntos_].length > 0) {
        const firstPendingPullInto = controller[pendingPullIntos_][0];
        if (firstPendingPullInto.bytesFilled > 0) {
          const error = new TypeError();
          readableByteStreamControllerError(controller, error);
          throw error;
        }
      }
      readableByteStreamControllerClearAlgorithms(controller);
      readableStreamClose(stream);
    }
    exports_84(
      "readableByteStreamControllerClose",
      readableByteStreamControllerClose
    );
    function readableByteStreamControllerCommitPullIntoDescriptor(
      stream,
      pullIntoDescriptor
    ) {
      // Assert: stream.[[state]] is not "errored".
      let done = false;
      if (stream[shared.state_] === "closed") {
        // Assert: pullIntoDescriptor.[[bytesFilled]] is 0.
        done = true;
      }
      const filledView = readableByteStreamControllerConvertPullIntoDescriptor(
        pullIntoDescriptor
      );
      if (pullIntoDescriptor.readerType === "default") {
        readableStreamFulfillReadRequest(stream, filledView, done);
      } else {
        // Assert: pullIntoDescriptor.[[readerType]] is "byob".
        readableStreamFulfillReadIntoRequest(stream, filledView, done);
      }
    }
    exports_84(
      "readableByteStreamControllerCommitPullIntoDescriptor",
      readableByteStreamControllerCommitPullIntoDescriptor
    );
    function readableByteStreamControllerConvertPullIntoDescriptor(
      pullIntoDescriptor
    ) {
      const { bytesFilled, elementSize } = pullIntoDescriptor;
      // Assert: bytesFilled <= pullIntoDescriptor.byteLength
      // Assert: bytesFilled mod elementSize is 0
      return new pullIntoDescriptor.ctor(
        pullIntoDescriptor.buffer,
        pullIntoDescriptor.byteOffset,
        bytesFilled / elementSize
      );
    }
    exports_84(
      "readableByteStreamControllerConvertPullIntoDescriptor",
      readableByteStreamControllerConvertPullIntoDescriptor
    );
    function readableByteStreamControllerEnqueue(controller, chunk) {
      const stream = controller[controlledReadableByteStream_];
      // Assert: controller.[[closeRequested]] is false.
      // Assert: stream.[[state]] is "readable".
      const { buffer, byteOffset, byteLength } = chunk;
      const transferredBuffer = shared.transferArrayBuffer(buffer);
      if (readableStreamHasDefaultReader(stream)) {
        if (readableStreamGetNumReadRequests(stream) === 0) {
          readableByteStreamControllerEnqueueChunkToQueue(
            controller,
            transferredBuffer,
            byteOffset,
            byteLength
          );
        } else {
          // Assert: controller.[[queue]] is empty.
          const transferredView = new Uint8Array(
            transferredBuffer,
            byteOffset,
            byteLength
          );
          readableStreamFulfillReadRequest(stream, transferredView, false);
        }
      } else if (readableStreamHasBYOBReader(stream)) {
        readableByteStreamControllerEnqueueChunkToQueue(
          controller,
          transferredBuffer,
          byteOffset,
          byteLength
        );
        readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue(
          controller
        );
      } else {
        // Assert: !IsReadableStreamLocked(stream) is false.
        readableByteStreamControllerEnqueueChunkToQueue(
          controller,
          transferredBuffer,
          byteOffset,
          byteLength
        );
      }
      readableByteStreamControllerCallPullIfNeeded(controller);
    }
    exports_84(
      "readableByteStreamControllerEnqueue",
      readableByteStreamControllerEnqueue
    );
    function readableByteStreamControllerEnqueueChunkToQueue(
      controller,
      buffer,
      byteOffset,
      byteLength
    ) {
      controller[q.queue_].push({ buffer, byteOffset, byteLength });
      controller[q.queueTotalSize_] += byteLength;
    }
    exports_84(
      "readableByteStreamControllerEnqueueChunkToQueue",
      readableByteStreamControllerEnqueueChunkToQueue
    );
    function readableByteStreamControllerError(controller, error) {
      const stream = controller[controlledReadableByteStream_];
      if (stream[shared.state_] !== "readable") {
        return;
      }
      readableByteStreamControllerClearPendingPullIntos(controller);
      q.resetQueue(controller);
      readableByteStreamControllerClearAlgorithms(controller);
      readableStreamError(stream, error);
    }
    exports_84(
      "readableByteStreamControllerError",
      readableByteStreamControllerError
    );
    function readableByteStreamControllerFillHeadPullIntoDescriptor(
      controller,
      size,
      pullIntoDescriptor
    ) {
      // Assert: either controller.[[pendingPullIntos]] is empty, or the first element of controller.[[pendingPullIntos]] is pullIntoDescriptor.
      readableByteStreamControllerInvalidateBYOBRequest(controller);
      pullIntoDescriptor.bytesFilled += size;
    }
    exports_84(
      "readableByteStreamControllerFillHeadPullIntoDescriptor",
      readableByteStreamControllerFillHeadPullIntoDescriptor
    );
    function readableByteStreamControllerFillPullIntoDescriptorFromQueue(
      controller,
      pullIntoDescriptor
    ) {
      const elementSize = pullIntoDescriptor.elementSize;
      const currentAlignedBytes =
        pullIntoDescriptor.bytesFilled -
        (pullIntoDescriptor.bytesFilled % elementSize);
      const maxBytesToCopy = Math.min(
        controller[q.queueTotalSize_],
        pullIntoDescriptor.byteLength - pullIntoDescriptor.bytesFilled
      );
      const maxBytesFilled = pullIntoDescriptor.bytesFilled + maxBytesToCopy;
      const maxAlignedBytes = maxBytesFilled - (maxBytesFilled % elementSize);
      let totalBytesToCopyRemaining = maxBytesToCopy;
      let ready = false;
      if (maxAlignedBytes > currentAlignedBytes) {
        totalBytesToCopyRemaining =
          maxAlignedBytes - pullIntoDescriptor.bytesFilled;
        ready = true;
      }
      const queue = controller[q.queue_];
      while (totalBytesToCopyRemaining > 0) {
        const headOfQueue = queue.front();
        const bytesToCopy = Math.min(
          totalBytesToCopyRemaining,
          headOfQueue.byteLength
        );
        const destStart =
          pullIntoDescriptor.byteOffset + pullIntoDescriptor.bytesFilled;
        shared.copyDataBlockBytes(
          pullIntoDescriptor.buffer,
          destStart,
          headOfQueue.buffer,
          headOfQueue.byteOffset,
          bytesToCopy
        );
        if (headOfQueue.byteLength === bytesToCopy) {
          queue.shift();
        } else {
          headOfQueue.byteOffset += bytesToCopy;
          headOfQueue.byteLength -= bytesToCopy;
        }
        controller[q.queueTotalSize_] -= bytesToCopy;
        readableByteStreamControllerFillHeadPullIntoDescriptor(
          controller,
          bytesToCopy,
          pullIntoDescriptor
        );
        totalBytesToCopyRemaining -= bytesToCopy;
      }
      if (!ready) {
        // Assert: controller[queueTotalSize_] === 0
        // Assert: pullIntoDescriptor.bytesFilled > 0
        // Assert: pullIntoDescriptor.bytesFilled < pullIntoDescriptor.elementSize
      }
      return ready;
    }
    exports_84(
      "readableByteStreamControllerFillPullIntoDescriptorFromQueue",
      readableByteStreamControllerFillPullIntoDescriptorFromQueue
    );
    function readableByteStreamControllerGetDesiredSize(controller) {
      const stream = controller[controlledReadableByteStream_];
      const state = stream[shared.state_];
      if (state === "errored") {
        return null;
      }
      if (state === "closed") {
        return 0;
      }
      return controller[strategyHWM_] - controller[q.queueTotalSize_];
    }
    exports_84(
      "readableByteStreamControllerGetDesiredSize",
      readableByteStreamControllerGetDesiredSize
    );
    function readableByteStreamControllerHandleQueueDrain(controller) {
      // Assert: controller.[[controlledReadableByteStream]].[[state]] is "readable".
      if (controller[q.queueTotalSize_] === 0 && controller[closeRequested_]) {
        readableByteStreamControllerClearAlgorithms(controller);
        readableStreamClose(controller[controlledReadableByteStream_]);
      } else {
        readableByteStreamControllerCallPullIfNeeded(controller);
      }
    }
    exports_84(
      "readableByteStreamControllerHandleQueueDrain",
      readableByteStreamControllerHandleQueueDrain
    );
    function readableByteStreamControllerInvalidateBYOBRequest(controller) {
      const byobRequest = controller[byobRequest_];
      if (byobRequest === undefined) {
        return;
      }
      byobRequest[associatedReadableByteStreamController_] = undefined;
      byobRequest[view_] = undefined;
      controller[byobRequest_] = undefined;
    }
    exports_84(
      "readableByteStreamControllerInvalidateBYOBRequest",
      readableByteStreamControllerInvalidateBYOBRequest
    );
    function readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue(
      controller
    ) {
      // Assert: controller.[[closeRequested]] is false.
      const pendingPullIntos = controller[pendingPullIntos_];
      while (pendingPullIntos.length > 0) {
        if (controller[q.queueTotalSize_] === 0) {
          return;
        }
        const pullIntoDescriptor = pendingPullIntos[0];
        if (
          readableByteStreamControllerFillPullIntoDescriptorFromQueue(
            controller,
            pullIntoDescriptor
          )
        ) {
          readableByteStreamControllerShiftPendingPullInto(controller);
          readableByteStreamControllerCommitPullIntoDescriptor(
            controller[controlledReadableByteStream_],
            pullIntoDescriptor
          );
        }
      }
    }
    exports_84(
      "readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue",
      readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue
    );
    function readableByteStreamControllerPullInto(
      controller,
      view,
      forAuthorCode
    ) {
      const stream = controller[controlledReadableByteStream_];
      const elementSize = view.BYTES_PER_ELEMENT || 1; // DataView exposes this in Webkit as 1, is not present in FF or Blink
      const ctor = view.constructor; // the typecast here is just for TS typing, it does not influence buffer creation
      const byteOffset = view.byteOffset;
      const byteLength = view.byteLength;
      const buffer = shared.transferArrayBuffer(view.buffer);
      const pullIntoDescriptor = {
        buffer,
        byteOffset,
        byteLength,
        bytesFilled: 0,
        elementSize,
        ctor,
        readerType: "byob",
      };
      if (controller[pendingPullIntos_].length > 0) {
        controller[pendingPullIntos_].push(pullIntoDescriptor);
        return readableStreamAddReadIntoRequest(stream, forAuthorCode);
      }
      if (stream[shared.state_] === "closed") {
        const emptyView = new ctor(
          pullIntoDescriptor.buffer,
          pullIntoDescriptor.byteOffset,
          0
        );
        return Promise.resolve(
          readableStreamCreateReadResult(emptyView, true, forAuthorCode)
        );
      }
      if (controller[q.queueTotalSize_] > 0) {
        if (
          readableByteStreamControllerFillPullIntoDescriptorFromQueue(
            controller,
            pullIntoDescriptor
          )
        ) {
          const filledView = readableByteStreamControllerConvertPullIntoDescriptor(
            pullIntoDescriptor
          );
          readableByteStreamControllerHandleQueueDrain(controller);
          return Promise.resolve(
            readableStreamCreateReadResult(filledView, false, forAuthorCode)
          );
        }
        if (controller[closeRequested_]) {
          const error = new TypeError();
          readableByteStreamControllerError(controller, error);
          return Promise.reject(error);
        }
      }
      controller[pendingPullIntos_].push(pullIntoDescriptor);
      const promise = readableStreamAddReadIntoRequest(stream, forAuthorCode);
      readableByteStreamControllerCallPullIfNeeded(controller);
      return promise;
    }
    exports_84(
      "readableByteStreamControllerPullInto",
      readableByteStreamControllerPullInto
    );
    function readableByteStreamControllerRespond(controller, bytesWritten) {
      bytesWritten = Number(bytesWritten);
      if (!shared.isFiniteNonNegativeNumber(bytesWritten)) {
        throw new RangeError(
          "bytesWritten must be a finite, non-negative number"
        );
      }
      // Assert: controller.[[pendingPullIntos]] is not empty.
      readableByteStreamControllerRespondInternal(controller, bytesWritten);
    }
    exports_84(
      "readableByteStreamControllerRespond",
      readableByteStreamControllerRespond
    );
    function readableByteStreamControllerRespondInClosedState(
      controller,
      firstDescriptor
    ) {
      firstDescriptor.buffer = shared.transferArrayBuffer(
        firstDescriptor.buffer
      );
      // Assert: firstDescriptor.[[bytesFilled]] is 0.
      const stream = controller[controlledReadableByteStream_];
      if (readableStreamHasBYOBReader(stream)) {
        while (readableStreamGetNumReadIntoRequests(stream) > 0) {
          const pullIntoDescriptor = readableByteStreamControllerShiftPendingPullInto(
            controller
          );
          readableByteStreamControllerCommitPullIntoDescriptor(
            stream,
            pullIntoDescriptor
          );
        }
      }
    }
    exports_84(
      "readableByteStreamControllerRespondInClosedState",
      readableByteStreamControllerRespondInClosedState
    );
    function readableByteStreamControllerRespondInReadableState(
      controller,
      bytesWritten,
      pullIntoDescriptor
    ) {
      if (
        pullIntoDescriptor.bytesFilled + bytesWritten >
        pullIntoDescriptor.byteLength
      ) {
        throw new RangeError();
      }
      readableByteStreamControllerFillHeadPullIntoDescriptor(
        controller,
        bytesWritten,
        pullIntoDescriptor
      );
      if (pullIntoDescriptor.bytesFilled < pullIntoDescriptor.elementSize) {
        return;
      }
      readableByteStreamControllerShiftPendingPullInto(controller);
      const remainderSize =
        pullIntoDescriptor.bytesFilled % pullIntoDescriptor.elementSize;
      if (remainderSize > 0) {
        const end =
          pullIntoDescriptor.byteOffset + pullIntoDescriptor.bytesFilled;
        const remainder = shared.cloneArrayBuffer(
          pullIntoDescriptor.buffer,
          end - remainderSize,
          remainderSize,
          ArrayBuffer
        );
        readableByteStreamControllerEnqueueChunkToQueue(
          controller,
          remainder,
          0,
          remainder.byteLength
        );
      }
      pullIntoDescriptor.buffer = shared.transferArrayBuffer(
        pullIntoDescriptor.buffer
      );
      pullIntoDescriptor.bytesFilled =
        pullIntoDescriptor.bytesFilled - remainderSize;
      readableByteStreamControllerCommitPullIntoDescriptor(
        controller[controlledReadableByteStream_],
        pullIntoDescriptor
      );
      readableByteStreamControllerProcessPullIntoDescriptorsUsingQueue(
        controller
      );
    }
    exports_84(
      "readableByteStreamControllerRespondInReadableState",
      readableByteStreamControllerRespondInReadableState
    );
    function readableByteStreamControllerRespondInternal(
      controller,
      bytesWritten
    ) {
      const firstDescriptor = controller[pendingPullIntos_][0];
      const stream = controller[controlledReadableByteStream_];
      if (stream[shared.state_] === "closed") {
        if (bytesWritten !== 0) {
          throw new TypeError();
        }
        readableByteStreamControllerRespondInClosedState(
          controller,
          firstDescriptor
        );
      } else {
        // Assert: stream.[[state]] is "readable".
        readableByteStreamControllerRespondInReadableState(
          controller,
          bytesWritten,
          firstDescriptor
        );
      }
      readableByteStreamControllerCallPullIfNeeded(controller);
    }
    exports_84(
      "readableByteStreamControllerRespondInternal",
      readableByteStreamControllerRespondInternal
    );
    function readableByteStreamControllerRespondWithNewView(controller, view) {
      // Assert: controller.[[pendingPullIntos]] is not empty.
      const firstDescriptor = controller[pendingPullIntos_][0];
      if (
        firstDescriptor.byteOffset + firstDescriptor.bytesFilled !==
        view.byteOffset
      ) {
        throw new RangeError();
      }
      if (firstDescriptor.byteLength !== view.byteLength) {
        throw new RangeError();
      }
      firstDescriptor.buffer = view.buffer;
      readableByteStreamControllerRespondInternal(controller, view.byteLength);
    }
    exports_84(
      "readableByteStreamControllerRespondWithNewView",
      readableByteStreamControllerRespondWithNewView
    );
    function readableByteStreamControllerShiftPendingPullInto(controller) {
      const descriptor = controller[pendingPullIntos_].shift();
      readableByteStreamControllerInvalidateBYOBRequest(controller);
      return descriptor;
    }
    exports_84(
      "readableByteStreamControllerShiftPendingPullInto",
      readableByteStreamControllerShiftPendingPullInto
    );
    function readableByteStreamControllerShouldCallPull(controller) {
      // Let stream be controller.[[controlledReadableByteStream]].
      const stream = controller[controlledReadableByteStream_];
      if (stream[shared.state_] !== "readable") {
        return false;
      }
      if (controller[closeRequested_]) {
        return false;
      }
      if (!controller[started_]) {
        return false;
      }
      if (
        readableStreamHasDefaultReader(stream) &&
        readableStreamGetNumReadRequests(stream) > 0
      ) {
        return true;
      }
      if (
        readableStreamHasBYOBReader(stream) &&
        readableStreamGetNumReadIntoRequests(stream) > 0
      ) {
        return true;
      }
      const desiredSize = readableByteStreamControllerGetDesiredSize(
        controller
      );
      // Assert: desiredSize is not null.
      return desiredSize > 0;
    }
    exports_84(
      "readableByteStreamControllerShouldCallPull",
      readableByteStreamControllerShouldCallPull
    );
    function setUpReadableStreamBYOBRequest(request, controller, view) {
      if (!isReadableByteStreamController(controller)) {
        throw new TypeError();
      }
      if (!ArrayBuffer.isView(view)) {
        throw new TypeError();
      }
      // Assert: !IsDetachedBuffer(view.[[ViewedArrayBuffer]]) is false.
      request[associatedReadableByteStreamController_] = controller;
      request[view_] = view;
    }
    exports_84(
      "setUpReadableStreamBYOBRequest",
      setUpReadableStreamBYOBRequest
    );
    return {
      setters: [
        function (shared_1) {
          shared = shared_1;
        },
        function (q_1) {
          q = q_1;
        },
      ],
      execute: function () {
        // ReadableStreamDefaultController
        exports_84(
          "controlledReadableStream_",
          (controlledReadableStream_ = Symbol("controlledReadableStream_"))
        );
        exports_84(
          "pullAlgorithm_",
          (pullAlgorithm_ = Symbol("pullAlgorithm_"))
        );
        exports_84(
          "cancelAlgorithm_",
          (cancelAlgorithm_ = Symbol("cancelAlgorithm_"))
        );
        exports_84(
          "strategySizeAlgorithm_",
          (strategySizeAlgorithm_ = Symbol("strategySizeAlgorithm_"))
        );
        exports_84("strategyHWM_", (strategyHWM_ = Symbol("strategyHWM_")));
        exports_84("started_", (started_ = Symbol("started_")));
        exports_84(
          "closeRequested_",
          (closeRequested_ = Symbol("closeRequested_"))
        );
        exports_84("pullAgain_", (pullAgain_ = Symbol("pullAgain_")));
        exports_84("pulling_", (pulling_ = Symbol("pulling_")));
        exports_84("cancelSteps_", (cancelSteps_ = Symbol("cancelSteps_")));
        exports_84("pullSteps_", (pullSteps_ = Symbol("pullSteps_")));
        // ReadableByteStreamController
        exports_84(
          "autoAllocateChunkSize_",
          (autoAllocateChunkSize_ = Symbol("autoAllocateChunkSize_"))
        );
        exports_84("byobRequest_", (byobRequest_ = Symbol("byobRequest_")));
        exports_84(
          "controlledReadableByteStream_",
          (controlledReadableByteStream_ = Symbol(
            "controlledReadableByteStream_"
          ))
        );
        exports_84(
          "pendingPullIntos_",
          (pendingPullIntos_ = Symbol("pendingPullIntos_"))
        );
        // ReadableStreamDefaultReader
        exports_84(
          "closedPromise_",
          (closedPromise_ = Symbol("closedPromise_"))
        );
        exports_84(
          "ownerReadableStream_",
          (ownerReadableStream_ = Symbol("ownerReadableStream_"))
        );
        exports_84("readRequests_", (readRequests_ = Symbol("readRequests_")));
        exports_84(
          "readIntoRequests_",
          (readIntoRequests_ = Symbol("readIntoRequests_"))
        );
        // ReadableStreamBYOBRequest
        exports_84(
          "associatedReadableByteStreamController_",
          (associatedReadableByteStreamController_ = Symbol(
            "associatedReadableByteStreamController_"
          ))
        );
        exports_84("view_", (view_ = Symbol("view_")));
        // ReadableStreamBYOBReader
        // ReadableStream
        exports_84("reader_", (reader_ = Symbol("reader_")));
        exports_84(
          "readableStreamController_",
          (readableStreamController_ = Symbol("readableStreamController_"))
        );
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream-default-controller.ts",
  [
    "$deno$/web/streams/readable-internals.ts",
    "$deno$/web/streams/shared-internals.ts",
    "$deno$/web/streams/queue-mixin.ts",
  ],
  function (exports_85, context_85) {
    "use strict";
    let rs, shared, q, ReadableStreamDefaultController;
    const __moduleName = context_85 && context_85.id;
    function setUpReadableStreamDefaultControllerFromUnderlyingSource(
      stream,
      underlyingSource,
      highWaterMark,
      sizeAlgorithm
    ) {
      // Assert: underlyingSource is not undefined.
      const controller = Object.create(
        ReadableStreamDefaultController.prototype
      );
      const startAlgorithm = () => {
        return shared.invokeOrNoop(underlyingSource, "start", [controller]);
      };
      const pullAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
        underlyingSource,
        "pull",
        [controller]
      );
      const cancelAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
        underlyingSource,
        "cancel",
        []
      );
      rs.setUpReadableStreamDefaultController(
        stream,
        controller,
        startAlgorithm,
        pullAlgorithm,
        cancelAlgorithm,
        highWaterMark,
        sizeAlgorithm
      );
    }
    exports_85(
      "setUpReadableStreamDefaultControllerFromUnderlyingSource",
      setUpReadableStreamDefaultControllerFromUnderlyingSource
    );
    return {
      setters: [
        function (rs_1) {
          rs = rs_1;
        },
        function (shared_2) {
          shared = shared_2;
        },
        function (q_2) {
          q = q_2;
        },
      ],
      execute: function () {
        ReadableStreamDefaultController = class ReadableStreamDefaultController {
          constructor() {
            throw new TypeError();
          }
          get desiredSize() {
            return rs.readableStreamDefaultControllerGetDesiredSize(this);
          }
          close() {
            if (!rs.isReadableStreamDefaultController(this)) {
              throw new TypeError();
            }
            if (!rs.readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
              throw new TypeError(
                "Cannot close, the stream is already closing or not readable"
              );
            }
            rs.readableStreamDefaultControllerClose(this);
          }
          enqueue(chunk) {
            if (!rs.isReadableStreamDefaultController(this)) {
              throw new TypeError();
            }
            if (!rs.readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
              throw new TypeError(
                "Cannot enqueue, the stream is closing or not readable"
              );
            }
            rs.readableStreamDefaultControllerEnqueue(this, chunk);
          }
          error(e) {
            if (!rs.isReadableStreamDefaultController(this)) {
              throw new TypeError();
            }
            rs.readableStreamDefaultControllerError(this, e);
          }
          [(rs.cancelAlgorithm_,
          rs.closeRequested_,
          rs.controlledReadableStream_,
          rs.pullAgain_,
          rs.pullAlgorithm_,
          rs.pulling_,
          rs.strategyHWM_,
          rs.strategySizeAlgorithm_,
          rs.started_,
          q.queue_,
          q.queueTotalSize_,
          rs.cancelSteps_)](reason) {
            q.resetQueue(this);
            const result = this[rs.cancelAlgorithm_](reason);
            rs.readableStreamDefaultControllerClearAlgorithms(this);
            return result;
          }
          [rs.pullSteps_](forAuthorCode) {
            const stream = this[rs.controlledReadableStream_];
            if (this[q.queue_].length > 0) {
              const chunk = q.dequeueValue(this);
              if (this[rs.closeRequested_] && this[q.queue_].length === 0) {
                rs.readableStreamDefaultControllerClearAlgorithms(this);
                rs.readableStreamClose(stream);
              } else {
                rs.readableStreamDefaultControllerCallPullIfNeeded(this);
              }
              return Promise.resolve(
                rs.readableStreamCreateReadResult(chunk, false, forAuthorCode)
              );
            }
            const pendingPromise = rs.readableStreamAddReadRequest(
              stream,
              forAuthorCode
            );
            rs.readableStreamDefaultControllerCallPullIfNeeded(this);
            return pendingPromise;
          }
        };
        exports_85(
          "ReadableStreamDefaultController",
          ReadableStreamDefaultController
        );
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream-default-reader.ts",
  ["$deno$/web/streams/readable-internals.ts"],
  function (exports_86, context_86) {
    "use strict";
    let rs, ReadableStreamDefaultReader;
    const __moduleName = context_86 && context_86.id;
    return {
      setters: [
        function (rs_2) {
          rs = rs_2;
        },
      ],
      execute: function () {
        ReadableStreamDefaultReader = class ReadableStreamDefaultReader {
          constructor(stream) {
            if (!rs.isReadableStream(stream)) {
              throw new TypeError();
            }
            if (rs.isReadableStreamLocked(stream)) {
              throw new TypeError("The stream is locked.");
            }
            rs.readableStreamReaderGenericInitialize(this, stream);
            this[rs.readRequests_] = [];
          }
          get closed() {
            if (!rs.isReadableStreamDefaultReader(this)) {
              return Promise.reject(new TypeError());
            }
            return this[rs.closedPromise_].promise;
          }
          cancel(reason) {
            if (!rs.isReadableStreamDefaultReader(this)) {
              return Promise.reject(new TypeError());
            }
            const stream = this[rs.ownerReadableStream_];
            if (stream === undefined) {
              return Promise.reject(
                new TypeError("Reader is not associated with a stream")
              );
            }
            return rs.readableStreamCancel(stream, reason);
          }
          read() {
            if (!rs.isReadableStreamDefaultReader(this)) {
              return Promise.reject(new TypeError());
            }
            if (this[rs.ownerReadableStream_] === undefined) {
              return Promise.reject(
                new TypeError("Reader is not associated with a stream")
              );
            }
            return rs.readableStreamDefaultReaderRead(this, true);
          }
          releaseLock() {
            if (!rs.isReadableStreamDefaultReader(this)) {
              throw new TypeError();
            }
            if (this[rs.ownerReadableStream_] === undefined) {
              return;
            }
            if (this[rs.readRequests_].length !== 0) {
              throw new TypeError(
                "Cannot release a stream with pending read requests"
              );
            }
            rs.readableStreamReaderGenericRelease(this);
          }
        };
        exports_86("ReadableStreamDefaultReader", ReadableStreamDefaultReader);
        rs.closedPromise_, rs.ownerReadableStream_, rs.readRequests_;
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream-byob-request.ts",
  ["$deno$/web/streams/readable-internals.ts"],
  function (exports_87, context_87) {
    "use strict";
    let rs, ReadableStreamBYOBRequest;
    const __moduleName = context_87 && context_87.id;
    return {
      setters: [
        function (rs_3) {
          rs = rs_3;
        },
      ],
      execute: function () {
        ReadableStreamBYOBRequest = class ReadableStreamBYOBRequest {
          constructor() {
            throw new TypeError();
          }
          get view() {
            if (!rs.isReadableStreamBYOBRequest(this)) {
              throw new TypeError();
            }
            return this[rs.view_];
          }
          respond(bytesWritten) {
            if (!rs.isReadableStreamBYOBRequest(this)) {
              throw new TypeError();
            }
            if (
              this[rs.associatedReadableByteStreamController_] === undefined
            ) {
              throw new TypeError();
            }
            // If! IsDetachedBuffer(this.[[view]].[[ViewedArrayBuffer]]) is true, throw a TypeError exception.
            return rs.readableByteStreamControllerRespond(
              this[rs.associatedReadableByteStreamController_],
              bytesWritten
            );
          }
          respondWithNewView(view) {
            if (!rs.isReadableStreamBYOBRequest(this)) {
              throw new TypeError();
            }
            if (
              this[rs.associatedReadableByteStreamController_] === undefined
            ) {
              throw new TypeError();
            }
            if (!ArrayBuffer.isView(view)) {
              throw new TypeError("view parameter must be a TypedArray");
            }
            // If! IsDetachedBuffer(view.[[ViewedArrayBuffer]]) is true, throw a TypeError exception.
            return rs.readableByteStreamControllerRespondWithNewView(
              this[rs.associatedReadableByteStreamController_],
              view
            );
          }
        };
        exports_87("ReadableStreamBYOBRequest", ReadableStreamBYOBRequest);
        rs.associatedReadableByteStreamController_, rs.view_;
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-byte-stream-controller.ts",
  [
    "$deno$/web/streams/readable-internals.ts",
    "$deno$/web/streams/queue-mixin.ts",
    "$deno$/web/streams/shared-internals.ts",
    "$deno$/web/streams/readable-stream-byob-request.ts",
  ],
  function (exports_88, context_88) {
    "use strict";
    let rs,
      q,
      shared,
      readable_stream_byob_request_ts_1,
      ReadableByteStreamController;
    const __moduleName = context_88 && context_88.id;
    function setUpReadableByteStreamControllerFromUnderlyingSource(
      stream,
      underlyingByteSource,
      highWaterMark
    ) {
      // Assert: underlyingByteSource is not undefined.
      const controller = Object.create(ReadableByteStreamController.prototype);
      const startAlgorithm = () => {
        return shared.invokeOrNoop(underlyingByteSource, "start", [controller]);
      };
      const pullAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
        underlyingByteSource,
        "pull",
        [controller]
      );
      const cancelAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
        underlyingByteSource,
        "cancel",
        []
      );
      let autoAllocateChunkSize = underlyingByteSource.autoAllocateChunkSize;
      if (autoAllocateChunkSize !== undefined) {
        autoAllocateChunkSize = Number(autoAllocateChunkSize);
        if (
          !shared.isInteger(autoAllocateChunkSize) ||
          autoAllocateChunkSize <= 0
        ) {
          throw new RangeError(
            "autoAllocateChunkSize must be a positive, finite integer"
          );
        }
      }
      rs.setUpReadableByteStreamController(
        stream,
        controller,
        startAlgorithm,
        pullAlgorithm,
        cancelAlgorithm,
        highWaterMark,
        autoAllocateChunkSize
      );
    }
    exports_88(
      "setUpReadableByteStreamControllerFromUnderlyingSource",
      setUpReadableByteStreamControllerFromUnderlyingSource
    );
    return {
      setters: [
        function (rs_4) {
          rs = rs_4;
        },
        function (q_3) {
          q = q_3;
        },
        function (shared_3) {
          shared = shared_3;
        },
        function (readable_stream_byob_request_ts_1_1) {
          readable_stream_byob_request_ts_1 = readable_stream_byob_request_ts_1_1;
        },
      ],
      execute: function () {
        ReadableByteStreamController = class ReadableByteStreamController {
          constructor() {
            throw new TypeError();
          }
          get byobRequest() {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            if (
              this[rs.byobRequest_] === undefined &&
              this[rs.pendingPullIntos_].length > 0
            ) {
              const firstDescriptor = this[rs.pendingPullIntos_][0];
              const view = new Uint8Array(
                firstDescriptor.buffer,
                firstDescriptor.byteOffset + firstDescriptor.bytesFilled,
                firstDescriptor.byteLength - firstDescriptor.bytesFilled
              );
              const byobRequest = Object.create(
                readable_stream_byob_request_ts_1.ReadableStreamBYOBRequest
                  .prototype
              );
              rs.setUpReadableStreamBYOBRequest(byobRequest, this, view);
              this[rs.byobRequest_] = byobRequest;
            }
            return this[rs.byobRequest_];
          }
          get desiredSize() {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            return rs.readableByteStreamControllerGetDesiredSize(this);
          }
          close() {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            if (this[rs.closeRequested_]) {
              throw new TypeError("Stream is already closing");
            }
            if (
              this[rs.controlledReadableByteStream_][shared.state_] !==
              "readable"
            ) {
              throw new TypeError("Stream is closed or errored");
            }
            rs.readableByteStreamControllerClose(this);
          }
          enqueue(chunk) {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            if (this[rs.closeRequested_]) {
              throw new TypeError("Stream is already closing");
            }
            if (
              this[rs.controlledReadableByteStream_][shared.state_] !==
              "readable"
            ) {
              throw new TypeError("Stream is closed or errored");
            }
            if (!ArrayBuffer.isView(chunk)) {
              throw new TypeError("chunk must be a valid ArrayBufferView");
            }
            // If ! IsDetachedBuffer(chunk.[[ViewedArrayBuffer]]) is true, throw a TypeError exception.
            return rs.readableByteStreamControllerEnqueue(this, chunk);
          }
          error(error) {
            if (!rs.isReadableByteStreamController(this)) {
              throw new TypeError();
            }
            rs.readableByteStreamControllerError(this, error);
          }
          [(rs.autoAllocateChunkSize_,
          rs.byobRequest_,
          rs.cancelAlgorithm_,
          rs.closeRequested_,
          rs.controlledReadableByteStream_,
          rs.pullAgain_,
          rs.pullAlgorithm_,
          rs.pulling_,
          rs.pendingPullIntos_,
          rs.started_,
          rs.strategyHWM_,
          q.queue_,
          q.queueTotalSize_,
          rs.cancelSteps_)](reason) {
            if (this[rs.pendingPullIntos_].length > 0) {
              const firstDescriptor = this[rs.pendingPullIntos_][0];
              firstDescriptor.bytesFilled = 0;
            }
            q.resetQueue(this);
            const result = this[rs.cancelAlgorithm_](reason);
            rs.readableByteStreamControllerClearAlgorithms(this);
            return result;
          }
          [rs.pullSteps_](forAuthorCode) {
            const stream = this[rs.controlledReadableByteStream_];
            // Assert: ! ReadableStreamHasDefaultReader(stream) is true.
            if (this[q.queueTotalSize_] > 0) {
              // Assert: ! ReadableStreamGetNumReadRequests(stream) is 0.
              const entry = this[q.queue_].shift();
              this[q.queueTotalSize_] -= entry.byteLength;
              rs.readableByteStreamControllerHandleQueueDrain(this);
              const view = new Uint8Array(
                entry.buffer,
                entry.byteOffset,
                entry.byteLength
              );
              return Promise.resolve(
                rs.readableStreamCreateReadResult(view, false, forAuthorCode)
              );
            }
            const autoAllocateChunkSize = this[rs.autoAllocateChunkSize_];
            if (autoAllocateChunkSize !== undefined) {
              let buffer;
              try {
                buffer = new ArrayBuffer(autoAllocateChunkSize);
              } catch (error) {
                return Promise.reject(error);
              }
              const pullIntoDescriptor = {
                buffer,
                byteOffset: 0,
                byteLength: autoAllocateChunkSize,
                bytesFilled: 0,
                elementSize: 1,
                ctor: Uint8Array,
                readerType: "default",
              };
              this[rs.pendingPullIntos_].push(pullIntoDescriptor);
            }
            const promise = rs.readableStreamAddReadRequest(
              stream,
              forAuthorCode
            );
            rs.readableByteStreamControllerCallPullIfNeeded(this);
            return promise;
          }
        };
        exports_88(
          "ReadableByteStreamController",
          ReadableByteStreamController
        );
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream-byob-reader.ts",
  ["$deno$/web/streams/readable-internals.ts"],
  function (exports_89, context_89) {
    "use strict";
    let rs, SDReadableStreamBYOBReader;
    const __moduleName = context_89 && context_89.id;
    return {
      setters: [
        function (rs_5) {
          rs = rs_5;
        },
      ],
      execute: function () {
        SDReadableStreamBYOBReader = class SDReadableStreamBYOBReader {
          constructor(stream) {
            if (!rs.isReadableStream(stream)) {
              throw new TypeError();
            }
            if (
              !rs.isReadableByteStreamController(
                stream[rs.readableStreamController_]
              )
            ) {
              throw new TypeError();
            }
            if (rs.isReadableStreamLocked(stream)) {
              throw new TypeError("The stream is locked.");
            }
            rs.readableStreamReaderGenericInitialize(this, stream);
            this[rs.readIntoRequests_] = [];
          }
          get closed() {
            if (!rs.isReadableStreamBYOBReader(this)) {
              return Promise.reject(new TypeError());
            }
            return this[rs.closedPromise_].promise;
          }
          cancel(reason) {
            if (!rs.isReadableStreamBYOBReader(this)) {
              return Promise.reject(new TypeError());
            }
            const stream = this[rs.ownerReadableStream_];
            if (stream === undefined) {
              return Promise.reject(
                new TypeError("Reader is not associated with a stream")
              );
            }
            return rs.readableStreamCancel(stream, reason);
          }
          read(view) {
            if (!rs.isReadableStreamBYOBReader(this)) {
              return Promise.reject(new TypeError());
            }
            if (this[rs.ownerReadableStream_] === undefined) {
              return Promise.reject(
                new TypeError("Reader is not associated with a stream")
              );
            }
            if (!ArrayBuffer.isView(view)) {
              return Promise.reject(
                new TypeError("view argument must be a valid ArrayBufferView")
              );
            }
            // If ! IsDetachedBuffer(view.[[ViewedArrayBuffer]]) is true, return a promise rejected with a TypeError exception.
            if (view.byteLength === 0) {
              return Promise.reject(
                new TypeError("supplied buffer view must be > 0 bytes")
              );
            }
            return rs.readableStreamBYOBReaderRead(this, view, true);
          }
          releaseLock() {
            if (!rs.isReadableStreamBYOBReader(this)) {
              throw new TypeError();
            }
            if (this[rs.ownerReadableStream_] === undefined) {
              throw new TypeError("Reader is not associated with a stream");
            }
            if (this[rs.readIntoRequests_].length > 0) {
              throw new TypeError();
            }
            rs.readableStreamReaderGenericRelease(this);
          }
        };
        exports_89("SDReadableStreamBYOBReader", SDReadableStreamBYOBReader);
        rs.closedPromise_, rs.ownerReadableStream_, rs.readIntoRequests_;
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/readable-stream.ts",
  [
    "$deno$/web/streams/readable-internals.ts",
    "$deno$/web/streams/shared-internals.ts",
    "$deno$/web/streams/readable-stream-default-controller.ts",
    "$deno$/web/streams/readable-stream-default-reader.ts",
    "$deno$/web/streams/readable-byte-stream-controller.ts",
    "$deno$/web/streams/readable-stream-byob-reader.ts",
  ],
  function (exports_90, context_90) {
    "use strict";
    let rs,
      shared,
      readable_stream_default_controller_ts_1,
      readable_stream_default_reader_ts_1,
      readable_byte_stream_controller_ts_1,
      readable_stream_byob_reader_ts_1,
      SDReadableStream;
    const __moduleName = context_90 && context_90.id;
    function createReadableStream(
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      sizeAlgorithm
    ) {
      if (highWaterMark === undefined) {
        highWaterMark = 1;
      }
      if (sizeAlgorithm === undefined) {
        sizeAlgorithm = () => 1;
      }
      // Assert: ! IsNonNegativeNumber(highWaterMark) is true.
      const stream = Object.create(SDReadableStream.prototype);
      rs.initializeReadableStream(stream);
      const controller = Object.create(
        readable_stream_default_controller_ts_1.ReadableStreamDefaultController
          .prototype
      );
      rs.setUpReadableStreamDefaultController(
        stream,
        controller,
        startAlgorithm,
        pullAlgorithm,
        cancelAlgorithm,
        highWaterMark,
        sizeAlgorithm
      );
      return stream;
    }
    exports_90("createReadableStream", createReadableStream);
    function createReadableByteStream(
      startAlgorithm,
      pullAlgorithm,
      cancelAlgorithm,
      highWaterMark,
      autoAllocateChunkSize
    ) {
      if (highWaterMark === undefined) {
        highWaterMark = 0;
      }
      // Assert: ! IsNonNegativeNumber(highWaterMark) is true.
      if (autoAllocateChunkSize !== undefined) {
        if (
          !shared.isInteger(autoAllocateChunkSize) ||
          autoAllocateChunkSize <= 0
        ) {
          throw new RangeError(
            "autoAllocateChunkSize must be a positive, finite integer"
          );
        }
      }
      const stream = Object.create(SDReadableStream.prototype);
      rs.initializeReadableStream(stream);
      const controller = Object.create(
        readable_byte_stream_controller_ts_1.ReadableByteStreamController
          .prototype
      );
      rs.setUpReadableByteStreamController(
        stream,
        controller,
        startAlgorithm,
        pullAlgorithm,
        cancelAlgorithm,
        highWaterMark,
        autoAllocateChunkSize
      );
      return stream;
    }
    exports_90("createReadableByteStream", createReadableByteStream);
    function readableStreamTee(stream, cloneForBranch2) {
      if (!rs.isReadableStream(stream)) {
        throw new TypeError();
      }
      const reader = new readable_stream_default_reader_ts_1.ReadableStreamDefaultReader(
        stream
      );
      let closedOrErrored = false;
      let canceled1 = false;
      let canceled2 = false;
      let reason1;
      let reason2;
      const branch1 = {};
      const branch2 = {};
      let cancelResolve;
      const cancelPromise = new Promise((resolve) => (cancelResolve = resolve));
      const pullAlgorithm = () => {
        return rs
          .readableStreamDefaultReaderRead(reader)
          .then(({ value, done }) => {
            if (done && !closedOrErrored) {
              if (!canceled1) {
                rs.readableStreamDefaultControllerClose(
                  branch1[rs.readableStreamController_]
                );
              }
              if (!canceled2) {
                rs.readableStreamDefaultControllerClose(
                  branch2[rs.readableStreamController_]
                );
              }
              closedOrErrored = true;
            }
            if (closedOrErrored) {
              return;
            }
            const value1 = value;
            let value2 = value;
            if (!canceled1) {
              rs.readableStreamDefaultControllerEnqueue(
                branch1[rs.readableStreamController_],
                value1
              );
            }
            if (!canceled2) {
              if (cloneForBranch2) {
                value2 = shared.cloneValue(value2);
              }
              rs.readableStreamDefaultControllerEnqueue(
                branch2[rs.readableStreamController_],
                value2
              );
            }
          });
      };
      const cancel1Algorithm = (reason) => {
        canceled1 = true;
        reason1 = reason;
        if (canceled2) {
          const cancelResult = rs.readableStreamCancel(stream, [
            reason1,
            reason2,
          ]);
          cancelResolve(cancelResult);
        }
        return cancelPromise;
      };
      const cancel2Algorithm = (reason) => {
        canceled2 = true;
        reason2 = reason;
        if (canceled1) {
          const cancelResult = rs.readableStreamCancel(stream, [
            reason1,
            reason2,
          ]);
          cancelResolve(cancelResult);
        }
        return cancelPromise;
      };
      const startAlgorithm = () => undefined;
      branch1 = createReadableStream(
        startAlgorithm,
        pullAlgorithm,
        cancel1Algorithm
      );
      branch2 = createReadableStream(
        startAlgorithm,
        pullAlgorithm,
        cancel2Algorithm
      );
      reader[rs.closedPromise_].promise.catch((error) => {
        if (!closedOrErrored) {
          rs.readableStreamDefaultControllerError(
            branch1[rs.readableStreamController_],
            error
          );
          rs.readableStreamDefaultControllerError(
            branch2[rs.readableStreamController_],
            error
          );
          closedOrErrored = true;
        }
      });
      return [branch1, branch2];
    }
    exports_90("readableStreamTee", readableStreamTee);
    return {
      setters: [
        function (rs_6) {
          rs = rs_6;
        },
        function (shared_4) {
          shared = shared_4;
        },
        function (readable_stream_default_controller_ts_1_1) {
          readable_stream_default_controller_ts_1 = readable_stream_default_controller_ts_1_1;
        },
        function (readable_stream_default_reader_ts_1_1) {
          readable_stream_default_reader_ts_1 = readable_stream_default_reader_ts_1_1;
        },
        function (readable_byte_stream_controller_ts_1_1) {
          readable_byte_stream_controller_ts_1 = readable_byte_stream_controller_ts_1_1;
        },
        function (readable_stream_byob_reader_ts_1_1) {
          readable_stream_byob_reader_ts_1 = readable_stream_byob_reader_ts_1_1;
        },
      ],
      execute: function () {
        SDReadableStream = class SDReadableStream {
          constructor(underlyingSource = {}, strategy = {}) {
            rs.initializeReadableStream(this);
            const sizeFunc = strategy.size;
            const stratHWM = strategy.highWaterMark;
            const sourceType = underlyingSource.type;
            if (sourceType === undefined) {
              const sizeAlgorithm = shared.makeSizeAlgorithmFromSizeFunction(
                sizeFunc
              );
              const highWaterMark = shared.validateAndNormalizeHighWaterMark(
                stratHWM === undefined ? 1 : stratHWM
              );
              readable_stream_default_controller_ts_1.setUpReadableStreamDefaultControllerFromUnderlyingSource(
                this,
                underlyingSource,
                highWaterMark,
                sizeAlgorithm
              );
            } else if (String(sourceType) === "bytes") {
              if (sizeFunc !== undefined) {
                throw new RangeError(
                  "bytes streams cannot have a strategy with a `size` field"
                );
              }
              const highWaterMark = shared.validateAndNormalizeHighWaterMark(
                stratHWM === undefined ? 0 : stratHWM
              );
              readable_byte_stream_controller_ts_1.setUpReadableByteStreamControllerFromUnderlyingSource(
                this,
                underlyingSource,
                highWaterMark
              );
            } else {
              throw new RangeError(
                "The underlying source's `type` field must be undefined or 'bytes'"
              );
            }
          }
          get locked() {
            return rs.isReadableStreamLocked(this);
          }
          getReader(options) {
            if (!rs.isReadableStream(this)) {
              throw new TypeError();
            }
            if (options === undefined) {
              options = {};
            }
            const { mode } = options;
            if (mode === undefined) {
              return new readable_stream_default_reader_ts_1.ReadableStreamDefaultReader(
                this
              );
            } else if (String(mode) === "byob") {
              return new readable_stream_byob_reader_ts_1.SDReadableStreamBYOBReader(
                this
              );
            }
            throw RangeError("mode option must be undefined or `byob`");
          }
          cancel(reason) {
            if (!rs.isReadableStream(this)) {
              return Promise.reject(new TypeError());
            }
            if (rs.isReadableStreamLocked(this)) {
              return Promise.reject(
                new TypeError("Cannot cancel a locked stream")
              );
            }
            return rs.readableStreamCancel(this, reason);
          }
          tee() {
            return readableStreamTee(this, false);
          }
        };
        exports_90("SDReadableStream", SDReadableStream);
        shared.state_,
          shared.storedError_,
          rs.reader_,
          rs.readableStreamController_;
      },
    };
  }
);
// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT
System.register(
  "$deno$/web/streams/mod.ts",
  ["$deno$/web/streams/readable-stream.ts"],
  function (exports_91, context_91) {
    "use strict";
    const __moduleName = context_91 && context_91.id;
    return {
      setters: [
        function (readable_stream_ts_1_1) {
          exports_91({
            ReadableStream: readable_stream_ts_1_1["SDReadableStream"],
          });
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/web/blob.ts",
  [
    "$deno$/web/text_encoding.ts",
    "$deno$/build.ts",
    "$deno$/web/streams/mod.ts",
  ],
  function (exports_92, context_92) {
    "use strict";
    let text_encoding_ts_8, build_ts_8, mod_ts_1, bytesSymbol, DenoBlob;
    const __moduleName = context_92 && context_92.id;
    function containsOnlyASCII(str) {
      if (typeof str !== "string") {
        return false;
      }
      return /^[\x00-\x7F]*$/.test(str);
    }
    exports_92("containsOnlyASCII", containsOnlyASCII);
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
      const enc = new text_encoding_ts_8.TextEncoder();
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
        function (text_encoding_ts_8_1) {
          text_encoding_ts_8 = text_encoding_ts_8_1;
        },
        function (build_ts_8_1) {
          build_ts_8 = build_ts_8_1;
        },
        function (mod_ts_1_1) {
          mod_ts_1 = mod_ts_1_1;
        },
      ],
      execute: function () {
        exports_92("bytesSymbol", (bytesSymbol = Symbol("bytes")));
        // A WeakMap holding blob to byte array mapping.
        // Ensures it does not impact garbage collection.
        exports_92("blobBytesWeakMap", new WeakMap());
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
            const decoder = new text_encoding_ts_8.TextDecoder();
            return decoder.decode(await readBytes(reader));
          }
          arrayBuffer() {
            return readBytes(getStream(this[bytesSymbol]).getReader());
          }
        };
        exports_92("DenoBlob", DenoBlob);
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/web/event.ts",
  ["$deno$/web/util.ts", "$deno$/util.ts"],
  function (exports_93, context_93) {
    "use strict";
    let util_ts_18, util_ts_19, eventData, EventImpl;
    const __moduleName = context_93 && context_93.id;
    // accessors for non runtime visible data
    function getDispatched(event) {
      return Boolean(eventData.get(event)?.dispatched);
    }
    exports_93("getDispatched", getDispatched);
    function getPath(event) {
      return eventData.get(event)?.path ?? [];
    }
    exports_93("getPath", getPath);
    function getStopImmediatePropagation(event) {
      return Boolean(eventData.get(event)?.stopImmediatePropagation);
    }
    exports_93("getStopImmediatePropagation", getStopImmediatePropagation);
    function setCurrentTarget(event, value) {
      event.currentTarget = value;
    }
    exports_93("setCurrentTarget", setCurrentTarget);
    function setDispatched(event, value) {
      const data = eventData.get(event);
      if (data) {
        data.dispatched = value;
      }
    }
    exports_93("setDispatched", setDispatched);
    function setEventPhase(event, value) {
      event.eventPhase = value;
    }
    exports_93("setEventPhase", setEventPhase);
    function setInPassiveListener(event, value) {
      const data = eventData.get(event);
      if (data) {
        data.inPassiveListener = value;
      }
    }
    exports_93("setInPassiveListener", setInPassiveListener);
    function setPath(event, value) {
      const data = eventData.get(event);
      if (data) {
        data.path = value;
      }
    }
    exports_93("setPath", setPath);
    function setRelatedTarget(event, value) {
      if ("relatedTarget" in event) {
        event.relatedTarget = value;
      }
    }
    exports_93("setRelatedTarget", setRelatedTarget);
    function setTarget(event, value) {
      event.target = value;
    }
    exports_93("setTarget", setTarget);
    function setStopImmediatePropagation(event, value) {
      const data = eventData.get(event);
      if (data) {
        data.stopImmediatePropagation = value;
      }
    }
    exports_93("setStopImmediatePropagation", setStopImmediatePropagation);
    // Type guards that widen the event type
    function hasRelatedTarget(event) {
      return "relatedTarget" in event;
    }
    exports_93("hasRelatedTarget", hasRelatedTarget);
    function isTrusted() {
      return eventData.get(this).isTrusted;
    }
    return {
      setters: [
        function (util_ts_18_1) {
          util_ts_18 = util_ts_18_1;
        },
        function (util_ts_19_1) {
          util_ts_19 = util_ts_19_1;
        },
      ],
      execute: function () {
        eventData = new WeakMap();
        EventImpl = class EventImpl {
          constructor(type, eventInitDict = {}) {
            this.#canceledFlag = false;
            this.#stopPropagationFlag = false;
            util_ts_18.requiredArguments("Event", arguments.length, 1);
            type = String(type);
            this.#attributes = {
              type,
              bubbles: eventInitDict.bubbles ?? false,
              cancelable: eventInitDict.cancelable ?? false,
              composed: eventInitDict.composed ?? false,
              currentTarget: null,
              eventPhase: Event.NONE,
              target: null,
              timeStamp: Date.now(),
            };
            eventData.set(this, {
              dispatched: false,
              inPassiveListener: false,
              isTrusted: false,
              path: [],
              stopImmediatePropagation: false,
            });
            Reflect.defineProperty(this, "isTrusted", {
              enumerable: true,
              get: isTrusted,
            });
          }
          #canceledFlag;
          #stopPropagationFlag;
          #attributes;
          get bubbles() {
            return this.#attributes.bubbles;
          }
          get cancelBubble() {
            return this.#stopPropagationFlag;
          }
          set cancelBubble(value) {
            this.#stopPropagationFlag = value;
          }
          get cancelable() {
            return this.#attributes.cancelable;
          }
          get composed() {
            return this.#attributes.composed;
          }
          get currentTarget() {
            return this.#attributes.currentTarget;
          }
          set currentTarget(value) {
            this.#attributes = {
              type: this.type,
              bubbles: this.bubbles,
              cancelable: this.cancelable,
              composed: this.composed,
              currentTarget: value,
              eventPhase: this.eventPhase,
              target: this.target,
              timeStamp: this.timeStamp,
            };
          }
          get defaultPrevented() {
            return this.#canceledFlag;
          }
          get eventPhase() {
            return this.#attributes.eventPhase;
          }
          set eventPhase(value) {
            this.#attributes = {
              type: this.type,
              bubbles: this.bubbles,
              cancelable: this.cancelable,
              composed: this.composed,
              currentTarget: this.currentTarget,
              eventPhase: value,
              target: this.target,
              timeStamp: this.timeStamp,
            };
          }
          get initialized() {
            return true;
          }
          get target() {
            return this.#attributes.target;
          }
          set target(value) {
            this.#attributes = {
              type: this.type,
              bubbles: this.bubbles,
              cancelable: this.cancelable,
              composed: this.composed,
              currentTarget: this.currentTarget,
              eventPhase: this.eventPhase,
              target: value,
              timeStamp: this.timeStamp,
            };
          }
          get timeStamp() {
            return this.#attributes.timeStamp;
          }
          get type() {
            return this.#attributes.type;
          }
          composedPath() {
            const path = eventData.get(this).path;
            if (path.length === 0) {
              return [];
            }
            util_ts_19.assert(this.currentTarget);
            const composedPath = [
              {
                item: this.currentTarget,
                itemInShadowTree: false,
                relatedTarget: null,
                rootOfClosedTree: false,
                slotInClosedTree: false,
                target: null,
                touchTargetList: [],
              },
            ];
            let currentTargetIndex = 0;
            let currentTargetHiddenSubtreeLevel = 0;
            for (let index = path.length - 1; index >= 0; index--) {
              const { item, rootOfClosedTree, slotInClosedTree } = path[index];
              if (rootOfClosedTree) {
                currentTargetHiddenSubtreeLevel++;
              }
              if (item === this.currentTarget) {
                currentTargetIndex = index;
                break;
              }
              if (slotInClosedTree) {
                currentTargetHiddenSubtreeLevel--;
              }
            }
            let currentHiddenLevel = currentTargetHiddenSubtreeLevel;
            let maxHiddenLevel = currentTargetHiddenSubtreeLevel;
            for (let i = currentTargetIndex - 1; i >= 0; i--) {
              const { item, rootOfClosedTree, slotInClosedTree } = path[i];
              if (rootOfClosedTree) {
                currentHiddenLevel++;
              }
              if (currentHiddenLevel <= maxHiddenLevel) {
                composedPath.unshift({
                  item,
                  itemInShadowTree: false,
                  relatedTarget: null,
                  rootOfClosedTree: false,
                  slotInClosedTree: false,
                  target: null,
                  touchTargetList: [],
                });
              }
              if (slotInClosedTree) {
                currentHiddenLevel--;
                if (currentHiddenLevel < maxHiddenLevel) {
                  maxHiddenLevel = currentHiddenLevel;
                }
              }
            }
            currentHiddenLevel = currentTargetHiddenSubtreeLevel;
            maxHiddenLevel = currentTargetHiddenSubtreeLevel;
            for (
              let index = currentTargetIndex + 1;
              index < path.length;
              index++
            ) {
              const { item, rootOfClosedTree, slotInClosedTree } = path[index];
              if (slotInClosedTree) {
                currentHiddenLevel++;
              }
              if (currentHiddenLevel <= maxHiddenLevel) {
                composedPath.push({
                  item,
                  itemInShadowTree: false,
                  relatedTarget: null,
                  rootOfClosedTree: false,
                  slotInClosedTree: false,
                  target: null,
                  touchTargetList: [],
                });
              }
              if (rootOfClosedTree) {
                currentHiddenLevel--;
                if (currentHiddenLevel < maxHiddenLevel) {
                  maxHiddenLevel = currentHiddenLevel;
                }
              }
            }
            return composedPath.map((p) => p.item);
          }
          preventDefault() {
            if (this.cancelable && !eventData.get(this).inPassiveListener) {
              this.#canceledFlag = true;
            }
          }
          stopPropagation() {
            this.#stopPropagationFlag = true;
          }
          stopImmediatePropagation() {
            this.#stopPropagationFlag = true;
            eventData.get(this).stopImmediatePropagation = true;
          }
          get NONE() {
            return Event.NONE;
          }
          get CAPTURING_PHASE() {
            return Event.CAPTURING_PHASE;
          }
          get AT_TARGET() {
            return Event.AT_TARGET;
          }
          get BUBBLING_PHASE() {
            return Event.BUBBLING_PHASE;
          }
          static get NONE() {
            return 0;
          }
          static get CAPTURING_PHASE() {
            return 1;
          }
          static get AT_TARGET() {
            return 2;
          }
          static get BUBBLING_PHASE() {
            return 3;
          }
        };
        exports_93("EventImpl", EventImpl);
        util_ts_18.defineEnumerableProps(EventImpl, [
          "bubbles",
          "cancelable",
          "composed",
          "currentTarget",
          "defaultPrevented",
          "eventPhase",
          "target",
          "timeStamp",
          "type",
        ]);
      },
    };
  }
);
System.register(
  "$deno$/web/custom_event.ts",
  ["$deno$/web/event.ts", "$deno$/web/util.ts"],
  function (exports_94, context_94) {
    "use strict";
    let event_ts_1, util_ts_20, CustomEventImpl;
    const __moduleName = context_94 && context_94.id;
    return {
      setters: [
        function (event_ts_1_1) {
          event_ts_1 = event_ts_1_1;
        },
        function (util_ts_20_1) {
          util_ts_20 = util_ts_20_1;
        },
      ],
      execute: function () {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        CustomEventImpl = class CustomEventImpl extends event_ts_1.EventImpl {
          constructor(type, eventInitDict = {}) {
            super(type, eventInitDict);
            util_ts_20.requiredArguments("CustomEvent", arguments.length, 1);
            const { detail } = eventInitDict;
            this.#detail = detail;
          }
          #detail;
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          get detail() {
            return this.#detail;
          }
          get [Symbol.toStringTag]() {
            return "CustomEvent";
          }
        };
        exports_94("CustomEventImpl", CustomEventImpl);
        Reflect.defineProperty(CustomEventImpl.prototype, "detail", {
          enumerable: true,
        });
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/web/dom_exception.ts", [], function (
  exports_95,
  context_95
) {
  "use strict";
  let DOMExceptionImpl;
  const __moduleName = context_95 && context_95.id;
  return {
    setters: [],
    execute: function () {
      DOMExceptionImpl = class DOMExceptionImpl extends Error {
        constructor(message = "", name = "Error") {
          super(message);
          this.#name = name;
        }
        #name;
        get name() {
          return this.#name;
        }
      };
      exports_95("DOMExceptionImpl", DOMExceptionImpl);
    },
  };
});
System.register("$deno$/web/dom_file.ts", ["$deno$/web/blob.ts"], function (
  exports_96,
  context_96
) {
  "use strict";
  let blob, DomFileImpl;
  const __moduleName = context_96 && context_96.id;
  return {
    setters: [
      function (blob_1) {
        blob = blob_1;
      },
    ],
    execute: function () {
      DomFileImpl = class DomFileImpl extends blob.DenoBlob {
        constructor(fileBits, fileName, options) {
          const { lastModified = Date.now(), ...blobPropertyBag } =
            options ?? {};
          super(fileBits, blobPropertyBag);
          // 4.1.2.1 Replace any "/" character (U+002F SOLIDUS)
          // with a ":" (U + 003A COLON)
          this.name = String(fileName).replace(/\u002F/g, "\u003A");
          // 4.1.3.3 If lastModified is not provided, set lastModified to the current
          // date and time represented in number of milliseconds since the Unix Epoch.
          this.lastModified = lastModified;
        }
      };
      exports_96("DomFileImpl", DomFileImpl);
    },
  };
});
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/web/event_target.ts",
  ["$deno$/web/dom_exception.ts", "$deno$/web/event.ts", "$deno$/web/util.ts"],
  function (exports_97, context_97) {
    "use strict";
    let dom_exception_ts_1,
      event_ts_2,
      util_ts_21,
      DOCUMENT_FRAGMENT_NODE,
      eventTargetData,
      EventTargetImpl;
    const __moduleName = context_97 && context_97.id;
    // DOM Logic Helper functions and type guards
    /** Get the parent node, for event targets that have a parent.
     *
     * Ref: https://dom.spec.whatwg.org/#get-the-parent */
    function getParent(eventTarget) {
      return isNode(eventTarget) ? eventTarget.parentNode : null;
    }
    function getRoot(eventTarget) {
      return isNode(eventTarget)
        ? eventTarget.getRootNode({ composed: true })
        : null;
    }
    function isNode(eventTarget) {
      return Boolean(eventTarget && "nodeType" in eventTarget);
    }
    // https://dom.spec.whatwg.org/#concept-shadow-including-inclusive-ancestor
    function isShadowInclusiveAncestor(ancestor, node) {
      while (isNode(node)) {
        if (node === ancestor) {
          return true;
        }
        if (isShadowRoot(node)) {
          node = node && getHost(node);
        } else {
          node = getParent(node);
        }
      }
      return false;
    }
    function isShadowRoot(nodeImpl) {
      return Boolean(
        nodeImpl &&
          isNode(nodeImpl) &&
          nodeImpl.nodeType === DOCUMENT_FRAGMENT_NODE &&
          getHost(nodeImpl) != null
      );
    }
    function isSlotable(nodeImpl) {
      return Boolean(isNode(nodeImpl) && "assignedSlot" in nodeImpl);
    }
    // DOM Logic functions
    /** Append a path item to an event's path.
     *
     * Ref: https://dom.spec.whatwg.org/#concept-event-path-append
     */
    function appendToEventPath(
      eventImpl,
      target,
      targetOverride,
      relatedTarget,
      touchTargets,
      slotInClosedTree
    ) {
      const itemInShadowTree = isNode(target) && isShadowRoot(getRoot(target));
      const rootOfClosedTree =
        isShadowRoot(target) && getMode(target) === "closed";
      event_ts_2.getPath(eventImpl).push({
        item: target,
        itemInShadowTree,
        target: targetOverride,
        relatedTarget,
        touchTargetList: touchTargets,
        rootOfClosedTree,
        slotInClosedTree,
      });
    }
    function dispatch(targetImpl, eventImpl, targetOverride) {
      let clearTargets = false;
      let activationTarget = null;
      event_ts_2.setDispatched(eventImpl, true);
      targetOverride = targetOverride ?? targetImpl;
      const eventRelatedTarget = event_ts_2.hasRelatedTarget(eventImpl)
        ? eventImpl.relatedTarget
        : null;
      let relatedTarget = retarget(eventRelatedTarget, targetImpl);
      if (targetImpl !== relatedTarget || targetImpl === eventRelatedTarget) {
        const touchTargets = [];
        appendToEventPath(
          eventImpl,
          targetImpl,
          targetOverride,
          relatedTarget,
          touchTargets,
          false
        );
        const isActivationEvent = eventImpl.type === "click";
        if (isActivationEvent && getHasActivationBehavior(targetImpl)) {
          activationTarget = targetImpl;
        }
        let slotInClosedTree = false;
        let slotable =
          isSlotable(targetImpl) && getAssignedSlot(targetImpl)
            ? targetImpl
            : null;
        let parent = getParent(targetImpl);
        // Populate event path
        // https://dom.spec.whatwg.org/#event-path
        while (parent !== null) {
          if (slotable !== null) {
            slotable = null;
            const parentRoot = getRoot(parent);
            if (
              isShadowRoot(parentRoot) &&
              parentRoot &&
              getMode(parentRoot) === "closed"
            ) {
              slotInClosedTree = true;
            }
          }
          relatedTarget = retarget(eventRelatedTarget, parent);
          if (
            isNode(parent) &&
            isShadowInclusiveAncestor(getRoot(targetImpl), parent)
          ) {
            appendToEventPath(
              eventImpl,
              parent,
              null,
              relatedTarget,
              touchTargets,
              slotInClosedTree
            );
          } else if (parent === relatedTarget) {
            parent = null;
          } else {
            targetImpl = parent;
            if (
              isActivationEvent &&
              activationTarget === null &&
              getHasActivationBehavior(targetImpl)
            ) {
              activationTarget = targetImpl;
            }
            appendToEventPath(
              eventImpl,
              parent,
              targetImpl,
              relatedTarget,
              touchTargets,
              slotInClosedTree
            );
          }
          if (parent !== null) {
            parent = getParent(parent);
          }
          slotInClosedTree = false;
        }
        let clearTargetsTupleIndex = -1;
        const path = event_ts_2.getPath(eventImpl);
        for (
          let i = path.length - 1;
          i >= 0 && clearTargetsTupleIndex === -1;
          i--
        ) {
          if (path[i].target !== null) {
            clearTargetsTupleIndex = i;
          }
        }
        const clearTargetsTuple = path[clearTargetsTupleIndex];
        clearTargets =
          (isNode(clearTargetsTuple.target) &&
            isShadowRoot(getRoot(clearTargetsTuple.target))) ||
          (isNode(clearTargetsTuple.relatedTarget) &&
            isShadowRoot(getRoot(clearTargetsTuple.relatedTarget)));
        event_ts_2.setEventPhase(
          eventImpl,
          event_ts_2.EventImpl.CAPTURING_PHASE
        );
        for (let i = path.length - 1; i >= 0; --i) {
          const tuple = path[i];
          if (tuple.target === null) {
            invokeEventListeners(tuple, eventImpl);
          }
        }
        for (let i = 0; i < path.length; i++) {
          const tuple = path[i];
          if (tuple.target !== null) {
            event_ts_2.setEventPhase(eventImpl, event_ts_2.EventImpl.AT_TARGET);
          } else {
            event_ts_2.setEventPhase(
              eventImpl,
              event_ts_2.EventImpl.BUBBLING_PHASE
            );
          }
          if (
            (eventImpl.eventPhase === event_ts_2.EventImpl.BUBBLING_PHASE &&
              eventImpl.bubbles) ||
            eventImpl.eventPhase === event_ts_2.EventImpl.AT_TARGET
          ) {
            invokeEventListeners(tuple, eventImpl);
          }
        }
      }
      event_ts_2.setEventPhase(eventImpl, event_ts_2.EventImpl.NONE);
      event_ts_2.setCurrentTarget(eventImpl, null);
      event_ts_2.setPath(eventImpl, []);
      event_ts_2.setDispatched(eventImpl, false);
      eventImpl.cancelBubble = false;
      event_ts_2.setStopImmediatePropagation(eventImpl, false);
      if (clearTargets) {
        event_ts_2.setTarget(eventImpl, null);
        event_ts_2.setRelatedTarget(eventImpl, null);
      }
      // TODO: invoke activation targets if HTML nodes will be implemented
      // if (activationTarget !== null) {
      //   if (!eventImpl.defaultPrevented) {
      //     activationTarget._activationBehavior();
      //   }
      // }
      return !eventImpl.defaultPrevented;
    }
    /** Inner invoking of the event listeners where the resolved listeners are
     * called.
     *
     * Ref: https://dom.spec.whatwg.org/#concept-event-listener-inner-invoke */
    function innerInvokeEventListeners(eventImpl, targetListeners) {
      let found = false;
      const { type } = eventImpl;
      if (!targetListeners || !targetListeners[type]) {
        return found;
      }
      // Copy event listeners before iterating since the list can be modified during the iteration.
      const handlers = targetListeners[type].slice();
      for (let i = 0; i < handlers.length; i++) {
        const listener = handlers[i];
        let capture, once, passive;
        if (typeof listener.options === "boolean") {
          capture = listener.options;
          once = false;
          passive = false;
        } else {
          capture = listener.options.capture;
          once = listener.options.once;
          passive = listener.options.passive;
        }
        // Check if the event listener has been removed since the listeners has been cloned.
        if (!targetListeners[type].includes(listener)) {
          continue;
        }
        found = true;
        if (
          (eventImpl.eventPhase === event_ts_2.EventImpl.CAPTURING_PHASE &&
            !capture) ||
          (eventImpl.eventPhase === event_ts_2.EventImpl.BUBBLING_PHASE &&
            capture)
        ) {
          continue;
        }
        if (once) {
          targetListeners[type].splice(
            targetListeners[type].indexOf(listener),
            1
          );
        }
        if (passive) {
          event_ts_2.setInPassiveListener(eventImpl, true);
        }
        if (typeof listener.callback === "object") {
          if (typeof listener.callback.handleEvent === "function") {
            listener.callback.handleEvent(eventImpl);
          }
        } else {
          listener.callback.call(eventImpl.currentTarget, eventImpl);
        }
        event_ts_2.setInPassiveListener(eventImpl, false);
        if (event_ts_2.getStopImmediatePropagation(eventImpl)) {
          return found;
        }
      }
      return found;
    }
    /** Invokes the listeners on a given event path with the supplied event.
     *
     * Ref: https://dom.spec.whatwg.org/#concept-event-listener-invoke */
    function invokeEventListeners(tuple, eventImpl) {
      const path = event_ts_2.getPath(eventImpl);
      const tupleIndex = path.indexOf(tuple);
      for (let i = tupleIndex; i >= 0; i--) {
        const t = path[i];
        if (t.target) {
          event_ts_2.setTarget(eventImpl, t.target);
          break;
        }
      }
      event_ts_2.setRelatedTarget(eventImpl, tuple.relatedTarget);
      if (eventImpl.cancelBubble) {
        return;
      }
      event_ts_2.setCurrentTarget(eventImpl, tuple.item);
      innerInvokeEventListeners(eventImpl, getListeners(tuple.item));
    }
    function normalizeAddEventHandlerOptions(options) {
      if (typeof options === "boolean" || typeof options === "undefined") {
        return {
          capture: Boolean(options),
          once: false,
          passive: false,
        };
      } else {
        return options;
      }
    }
    function normalizeEventHandlerOptions(options) {
      if (typeof options === "boolean" || typeof options === "undefined") {
        return {
          capture: Boolean(options),
        };
      } else {
        return options;
      }
    }
    /** Retarget the target following the spec logic.
     *
     * Ref: https://dom.spec.whatwg.org/#retarget */
    function retarget(a, b) {
      while (true) {
        if (!isNode(a)) {
          return a;
        }
        const aRoot = a.getRootNode();
        if (aRoot) {
          if (
            !isShadowRoot(aRoot) ||
            (isNode(b) && isShadowInclusiveAncestor(aRoot, b))
          ) {
            return a;
          }
          a = getHost(aRoot);
        }
      }
    }
    function getAssignedSlot(target) {
      return Boolean(eventTargetData.get(target)?.assignedSlot);
    }
    function getHasActivationBehavior(target) {
      return Boolean(eventTargetData.get(target)?.hasActivationBehavior);
    }
    function getHost(target) {
      return eventTargetData.get(target)?.host ?? null;
    }
    function getListeners(target) {
      return eventTargetData.get(target)?.listeners ?? {};
    }
    function getMode(target) {
      return eventTargetData.get(target)?.mode ?? null;
    }
    function getDefaultTargetData() {
      return {
        assignedSlot: false,
        hasActivationBehavior: false,
        host: null,
        listeners: Object.create(null),
        mode: "",
      };
    }
    exports_97("getDefaultTargetData", getDefaultTargetData);
    return {
      setters: [
        function (dom_exception_ts_1_1) {
          dom_exception_ts_1 = dom_exception_ts_1_1;
        },
        function (event_ts_2_1) {
          event_ts_2 = event_ts_2_1;
        },
        function (util_ts_21_1) {
          util_ts_21 = util_ts_21_1;
        },
      ],
      execute: function () {
        // This is currently the only node type we are using, so instead of implementing
        // the whole of the Node interface at the moment, this just gives us the one
        // value to power the standards based logic
        DOCUMENT_FRAGMENT_NODE = 11;
        // Accessors for non-public data
        exports_97("eventTargetData", (eventTargetData = new WeakMap()));
        EventTargetImpl = class EventTargetImpl {
          constructor() {
            eventTargetData.set(this, getDefaultTargetData());
          }
          addEventListener(type, callback, options) {
            util_ts_21.requiredArguments(
              "EventTarget.addEventListener",
              arguments.length,
              2
            );
            if (callback === null) {
              return;
            }
            options = normalizeAddEventHandlerOptions(options);
            const { listeners } = eventTargetData.get(this ?? globalThis);
            if (!(type in listeners)) {
              listeners[type] = [];
            }
            for (const listener of listeners[type]) {
              if (
                ((typeof listener.options === "boolean" &&
                  listener.options === options.capture) ||
                  (typeof listener.options === "object" &&
                    listener.options.capture === options.capture)) &&
                listener.callback === callback
              ) {
                return;
              }
            }
            listeners[type].push({ callback, options });
          }
          removeEventListener(type, callback, options) {
            util_ts_21.requiredArguments(
              "EventTarget.removeEventListener",
              arguments.length,
              2
            );
            const listeners = eventTargetData.get(this ?? globalThis).listeners;
            if (callback !== null && type in listeners) {
              listeners[type] = listeners[type].filter(
                (listener) => listener.callback !== callback
              );
            } else if (callback === null || !listeners[type]) {
              return;
            }
            options = normalizeEventHandlerOptions(options);
            for (let i = 0; i < listeners[type].length; ++i) {
              const listener = listeners[type][i];
              if (
                ((typeof listener.options === "boolean" &&
                  listener.options === options.capture) ||
                  (typeof listener.options === "object" &&
                    listener.options.capture === options.capture)) &&
                listener.callback === callback
              ) {
                listeners[type].splice(i, 1);
                break;
              }
            }
          }
          dispatchEvent(event) {
            util_ts_21.requiredArguments(
              "EventTarget.dispatchEvent",
              arguments.length,
              1
            );
            const self = this ?? globalThis;
            const listeners = eventTargetData.get(self).listeners;
            if (!(event.type in listeners)) {
              return true;
            }
            if (event_ts_2.getDispatched(event)) {
              throw new dom_exception_ts_1.DOMExceptionImpl(
                "Invalid event state.",
                "InvalidStateError"
              );
            }
            if (event.eventPhase !== event_ts_2.EventImpl.NONE) {
              throw new dom_exception_ts_1.DOMExceptionImpl(
                "Invalid event state.",
                "InvalidStateError"
              );
            }
            return dispatch(self, event);
          }
          get [Symbol.toStringTag]() {
            return "EventTarget";
          }
          getParent(_event) {
            return null;
          }
        };
        exports_97("EventTargetImpl", EventTargetImpl);
        util_ts_21.defineEnumerableProps(EventTargetImpl, [
          "addEventListener",
          "removeEventListener",
          "dispatchEvent",
        ]);
      },
    };
  }
);
System.register(
  "$deno$/web/dom_iterable.ts",
  ["$deno$/web/util.ts", "$deno$/internals.ts"],
  function (exports_98, context_98) {
    "use strict";
    let util_ts_22, internals_ts_5;
    const __moduleName = context_98 && context_98.id;
    function DomIterableMixin(Base, dataSymbol) {
      // we have to cast `this` as `any` because there is no way to describe the
      // Base class in a way where the Symbol `dataSymbol` is defined.  So the
      // runtime code works, but we do lose a little bit of type safety.
      // Additionally, we have to not use .keys() nor .values() since the internal
      // slot differs in type - some have a Map, which yields [K, V] in
      // Symbol.iterator, and some have an Array, which yields V, in this case
      // [K, V] too as they are arrays of tuples.
      const DomIterable = class extends Base {
        *entries() {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          for (const entry of this[dataSymbol]) {
            yield entry;
          }
        }
        *keys() {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          for (const [key] of this[dataSymbol]) {
            yield key;
          }
        }
        *values() {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          for (const [, value] of this[dataSymbol]) {
            yield value;
          }
        }
        forEach(
          callbackfn,
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          thisArg
        ) {
          util_ts_22.requiredArguments(
            `${this.constructor.name}.forEach`,
            arguments.length,
            1
          );
          callbackfn = callbackfn.bind(
            thisArg == null ? globalThis : Object(thisArg)
          );
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          for (const [key, value] of this[dataSymbol]) {
            callbackfn(value, key, this);
          }
        }
        *[Symbol.iterator]() {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          for (const entry of this[dataSymbol]) {
            yield entry;
          }
        }
      };
      // we want the Base class name to be the name of the class.
      Object.defineProperty(DomIterable, "name", {
        value: Base.name,
        configurable: true,
      });
      return DomIterable;
    }
    exports_98("DomIterableMixin", DomIterableMixin);
    return {
      setters: [
        function (util_ts_22_1) {
          util_ts_22 = util_ts_22_1;
        },
        function (internals_ts_5_1) {
          internals_ts_5 = internals_ts_5_1;
        },
      ],
      execute: function () {
        internals_ts_5.exposeForTest("DomIterableMixin", DomIterableMixin);
      },
    };
  }
);
System.register(
  "$deno$/web/form_data.ts",
  [
    "$deno$/web/blob.ts",
    "$deno$/web/dom_file.ts",
    "$deno$/web/dom_iterable.ts",
    "$deno$/web/util.ts",
  ],
  function (exports_99, context_99) {
    "use strict";
    let _a,
      blob,
      domFile,
      dom_iterable_ts_1,
      util_ts_23,
      dataSymbol,
      FormDataBase,
      FormDataImpl;
    const __moduleName = context_99 && context_99.id;
    return {
      setters: [
        function (blob_2) {
          blob = blob_2;
        },
        function (domFile_1) {
          domFile = domFile_1;
        },
        function (dom_iterable_ts_1_1) {
          dom_iterable_ts_1 = dom_iterable_ts_1_1;
        },
        function (util_ts_23_1) {
          util_ts_23 = util_ts_23_1;
        },
      ],
      execute: function () {
        dataSymbol = Symbol("data");
        FormDataBase = class FormDataBase {
          constructor() {
            this[_a] = [];
          }
          append(name, value, filename) {
            util_ts_23.requiredArguments(
              "FormData.append",
              arguments.length,
              2
            );
            name = String(name);
            if (value instanceof domFile.DomFileImpl) {
              this[dataSymbol].push([name, value]);
            } else if (value instanceof blob.DenoBlob) {
              const dfile = new domFile.DomFileImpl([value], filename || name, {
                type: value.type,
              });
              this[dataSymbol].push([name, dfile]);
            } else {
              this[dataSymbol].push([name, String(value)]);
            }
          }
          delete(name) {
            util_ts_23.requiredArguments(
              "FormData.delete",
              arguments.length,
              1
            );
            name = String(name);
            let i = 0;
            while (i < this[dataSymbol].length) {
              if (this[dataSymbol][i][0] === name) {
                this[dataSymbol].splice(i, 1);
              } else {
                i++;
              }
            }
          }
          getAll(name) {
            util_ts_23.requiredArguments(
              "FormData.getAll",
              arguments.length,
              1
            );
            name = String(name);
            const values = [];
            for (const entry of this[dataSymbol]) {
              if (entry[0] === name) {
                values.push(entry[1]);
              }
            }
            return values;
          }
          get(name) {
            util_ts_23.requiredArguments("FormData.get", arguments.length, 1);
            name = String(name);
            for (const entry of this[dataSymbol]) {
              if (entry[0] === name) {
                return entry[1];
              }
            }
            return null;
          }
          has(name) {
            util_ts_23.requiredArguments("FormData.has", arguments.length, 1);
            name = String(name);
            return this[dataSymbol].some((entry) => entry[0] === name);
          }
          set(name, value, filename) {
            util_ts_23.requiredArguments("FormData.set", arguments.length, 2);
            name = String(name);
            // If there are any entries in the context object’s entry list whose name
            // is name, replace the first such entry with entry and remove the others
            let found = false;
            let i = 0;
            while (i < this[dataSymbol].length) {
              if (this[dataSymbol][i][0] === name) {
                if (!found) {
                  if (value instanceof domFile.DomFileImpl) {
                    this[dataSymbol][i][1] = value;
                  } else if (value instanceof blob.DenoBlob) {
                    const dfile = new domFile.DomFileImpl(
                      [value],
                      filename || name,
                      {
                        type: value.type,
                      }
                    );
                    this[dataSymbol][i][1] = dfile;
                  } else {
                    this[dataSymbol][i][1] = String(value);
                  }
                  found = true;
                } else {
                  this[dataSymbol].splice(i, 1);
                  continue;
                }
              }
              i++;
            }
            // Otherwise, append entry to the context object’s entry list.
            if (!found) {
              if (value instanceof domFile.DomFileImpl) {
                this[dataSymbol].push([name, value]);
              } else if (value instanceof blob.DenoBlob) {
                const dfile = new domFile.DomFileImpl(
                  [value],
                  filename || name,
                  {
                    type: value.type,
                  }
                );
                this[dataSymbol].push([name, dfile]);
              } else {
                this[dataSymbol].push([name, String(value)]);
              }
            }
          }
          get [((_a = dataSymbol), Symbol.toStringTag)]() {
            return "FormData";
          }
        };
        FormDataImpl = class FormDataImpl extends dom_iterable_ts_1.DomIterableMixin(
          FormDataBase,
          dataSymbol
        ) {};
        exports_99("FormDataImpl", FormDataImpl);
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/fetch.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_100, context_100) {
    "use strict";
    let dispatch_json_ts_35;
    const __moduleName = context_100 && context_100.id;
    function fetch(args, body) {
      let zeroCopy = undefined;
      if (body) {
        zeroCopy = new Uint8Array(
          body.buffer,
          body.byteOffset,
          body.byteLength
        );
      }
      return dispatch_json_ts_35.sendAsync("op_fetch", args, zeroCopy);
    }
    exports_100("fetch", fetch);
    return {
      setters: [
        function (dispatch_json_ts_35_1) {
          dispatch_json_ts_35 = dispatch_json_ts_35_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/web/fetch.ts",
  [
    "$deno$/util.ts",
    "$deno$/web/util.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/web/blob.ts",
    "$deno$/io.ts",
    "$deno$/ops/io.ts",
    "$deno$/ops/resources.ts",
    "$deno$/buffer.ts",
    "$deno$/ops/fetch.ts",
    "$deno$/web/dom_file.ts",
  ],
  function (exports_101, context_101) {
    "use strict";
    let util_ts_24,
      util_ts_25,
      text_encoding_ts_9,
      blob_ts_1,
      io,
      io_ts_7,
      resources_ts_7,
      buffer_ts_5,
      fetch_ts_1,
      dom_file_ts_1,
      Body,
      Response;
    const __moduleName = context_101 && context_101.id;
    function getHeaderValueParams(value) {
      const params = new Map();
      // Forced to do so for some Map constructor param mismatch
      value
        .split(";")
        .slice(1)
        .map((s) => s.trim().split("="))
        .filter((arr) => arr.length > 1)
        .map(([k, v]) => [k, v.replace(/^"([^"]*)"$/, "$1")])
        .forEach(([k, v]) => params.set(k, v));
      return params;
    }
    function hasHeaderValueOf(s, value) {
      return new RegExp(`^${value}[\t\s]*;?`).test(s);
    }
    function sendFetchReq(url, method, headers, body) {
      let headerArray = [];
      if (headers) {
        headerArray = Array.from(headers.entries());
      }
      const args = {
        method,
        url,
        headers: headerArray,
      };
      return fetch_ts_1.fetch(args, body);
    }
    async function fetch(input, init) {
      let url;
      let method = null;
      let headers = null;
      let body;
      let redirected = false;
      let remRedirectCount = 20; // TODO: use a better way to handle
      if (typeof input === "string" || input instanceof URL) {
        url = typeof input === "string" ? input : input.href;
        if (init != null) {
          method = init.method || null;
          if (init.headers) {
            headers =
              init.headers instanceof Headers
                ? init.headers
                : new Headers(init.headers);
          } else {
            headers = null;
          }
          // ref: https://fetch.spec.whatwg.org/#body-mixin
          // Body should have been a mixin
          // but we are treating it as a separate class
          if (init.body) {
            if (!headers) {
              headers = new Headers();
            }
            let contentType = "";
            if (typeof init.body === "string") {
              body = new text_encoding_ts_9.TextEncoder().encode(init.body);
              contentType = "text/plain;charset=UTF-8";
            } else if (util_ts_25.isTypedArray(init.body)) {
              body = init.body;
            } else if (init.body instanceof URLSearchParams) {
              body = new text_encoding_ts_9.TextEncoder().encode(
                init.body.toString()
              );
              contentType = "application/x-www-form-urlencoded;charset=UTF-8";
            } else if (init.body instanceof blob_ts_1.DenoBlob) {
              body = init.body[blob_ts_1.bytesSymbol];
              contentType = init.body.type;
            } else if (init.body instanceof FormData) {
              let boundary = "";
              if (headers.has("content-type")) {
                const params = getHeaderValueParams("content-type");
                if (params.has("boundary")) {
                  boundary = params.get("boundary");
                }
              }
              if (!boundary) {
                boundary =
                  "----------" +
                  Array.from(Array(32))
                    .map(() => Math.random().toString(36)[2] || 0)
                    .join("");
              }
              let payload = "";
              for (const [fieldName, fieldValue] of init.body.entries()) {
                let part = `\r\n--${boundary}\r\n`;
                part += `Content-Disposition: form-data; name=\"${fieldName}\"`;
                if (fieldValue instanceof dom_file_ts_1.DomFileImpl) {
                  part += `; filename=\"${fieldValue.name}\"`;
                }
                part += "\r\n";
                if (fieldValue instanceof dom_file_ts_1.DomFileImpl) {
                  part += `Content-Type: ${
                    fieldValue.type || "application/octet-stream"
                  }\r\n`;
                }
                part += "\r\n";
                if (fieldValue instanceof dom_file_ts_1.DomFileImpl) {
                  part += new text_encoding_ts_9.TextDecoder().decode(
                    fieldValue[blob_ts_1.bytesSymbol]
                  );
                } else {
                  part += fieldValue;
                }
                payload += part;
              }
              payload += `\r\n--${boundary}--`;
              body = new text_encoding_ts_9.TextEncoder().encode(payload);
              contentType = "multipart/form-data; boundary=" + boundary;
            } else {
              // TODO: ReadableStream
              util_ts_24.notImplemented();
            }
            if (contentType && !headers.has("content-type")) {
              headers.set("content-type", contentType);
            }
          }
        }
      } else {
        url = input.url;
        method = input.method;
        headers = input.headers;
        //@ts-ignore
        if (input._bodySource) {
          body = new DataView(await input.arrayBuffer());
        }
      }
      while (remRedirectCount) {
        const fetchResponse = await sendFetchReq(url, method, headers, body);
        const response = new Response(
          url,
          fetchResponse.status,
          fetchResponse.statusText,
          fetchResponse.headers,
          fetchResponse.bodyRid,
          redirected
        );
        if ([301, 302, 303, 307, 308].includes(response.status)) {
          // We won't use body of received response, so close it now
          // otherwise it will be kept in resource table.
          resources_ts_7.close(fetchResponse.bodyRid);
          // We're in a redirect status
          switch ((init && init.redirect) || "follow") {
            case "error":
              /* I suspect that deno will probably crash if you try to use that
                           rid, which suggests to me that Response needs to be refactored */
              return new Response("", 0, "", [], -1, false, "error", null);
            case "manual":
              return new Response(
                "",
                0,
                "",
                [],
                -1,
                false,
                "opaqueredirect",
                null
              );
            case "follow":
            default:
              let redirectUrl = response.headers.get("Location");
              if (redirectUrl == null) {
                return response; // Unspecified
              }
              if (
                !redirectUrl.startsWith("http://") &&
                !redirectUrl.startsWith("https://")
              ) {
                redirectUrl =
                  url.split("//")[0] +
                  "//" +
                  url.split("//")[1].split("/")[0] +
                  redirectUrl; // TODO: handle relative redirection more gracefully
              }
              url = redirectUrl;
              redirected = true;
              remRedirectCount--;
          }
        } else {
          return response;
        }
      }
      // Return a network error due to too many redirections
      throw util_ts_24.notImplemented();
    }
    exports_101("fetch", fetch);
    return {
      setters: [
        function (util_ts_24_1) {
          util_ts_24 = util_ts_24_1;
        },
        function (util_ts_25_1) {
          util_ts_25 = util_ts_25_1;
        },
        function (text_encoding_ts_9_1) {
          text_encoding_ts_9 = text_encoding_ts_9_1;
        },
        function (blob_ts_1_1) {
          blob_ts_1 = blob_ts_1_1;
        },
        function (io_1) {
          io = io_1;
        },
        function (io_ts_7_1) {
          io_ts_7 = io_ts_7_1;
        },
        function (resources_ts_7_1) {
          resources_ts_7 = resources_ts_7_1;
        },
        function (buffer_ts_5_1) {
          buffer_ts_5 = buffer_ts_5_1;
        },
        function (fetch_ts_1_1) {
          fetch_ts_1 = fetch_ts_1_1;
        },
        function (dom_file_ts_1_1) {
          dom_file_ts_1 = dom_file_ts_1_1;
        },
      ],
      execute: function () {
        Body = class Body {
          constructor(rid, contentType) {
            this.contentType = contentType;
            this.#bodyUsed = false;
            this.#bodyPromise = null;
            this.#data = null;
            this.locked = false; // TODO
            this.#bodyBuffer = async () => {
              util_ts_24.assert(this.#bodyPromise == null);
              const buf = new buffer_ts_5.Buffer();
              try {
                const nread = await buf.readFrom(this);
                const ui8 = buf.bytes();
                util_ts_24.assert(ui8.byteLength === nread);
                this.#data = ui8.buffer.slice(
                  ui8.byteOffset,
                  ui8.byteOffset + nread
                );
                util_ts_24.assert(this.#data.byteLength === nread);
              } finally {
                this.close();
              }
              return this.#data;
            };
            this.#rid = rid;
            this.body = this;
          }
          #bodyUsed;
          #bodyPromise;
          #data;
          #rid;
          #bodyBuffer;
          // eslint-disable-next-line require-await
          async arrayBuffer() {
            // If we've already bufferred the response, just return it.
            if (this.#data != null) {
              return this.#data;
            }
            // If there is no _bodyPromise yet, start it.
            if (this.#bodyPromise == null) {
              this.#bodyPromise = this.#bodyBuffer();
            }
            return this.#bodyPromise;
          }
          async blob() {
            const arrayBuffer = await this.arrayBuffer();
            return new blob_ts_1.DenoBlob([arrayBuffer], {
              type: this.contentType,
            });
          }
          // ref: https://fetch.spec.whatwg.org/#body-mixin
          async formData() {
            const formData = new FormData();
            const enc = new text_encoding_ts_9.TextEncoder();
            if (hasHeaderValueOf(this.contentType, "multipart/form-data")) {
              const params = getHeaderValueParams(this.contentType);
              if (!params.has("boundary")) {
                // TypeError is required by spec
                throw new TypeError(
                  "multipart/form-data must provide a boundary"
                );
              }
              // ref: https://tools.ietf.org/html/rfc2046#section-5.1
              const boundary = params.get("boundary");
              const dashBoundary = `--${boundary}`;
              const delimiter = `\r\n${dashBoundary}`;
              const closeDelimiter = `${delimiter}--`;
              const body = await this.text();
              let bodyParts;
              const bodyEpilogueSplit = body.split(closeDelimiter);
              if (bodyEpilogueSplit.length < 2) {
                bodyParts = [];
              } else {
                // discard epilogue
                const bodyEpilogueTrimmed = bodyEpilogueSplit[0];
                // first boundary treated special due to optional prefixed \r\n
                const firstBoundaryIndex = bodyEpilogueTrimmed.indexOf(
                  dashBoundary
                );
                if (firstBoundaryIndex < 0) {
                  throw new TypeError("Invalid boundary");
                }
                const bodyPreambleTrimmed = bodyEpilogueTrimmed
                  .slice(firstBoundaryIndex + dashBoundary.length)
                  .replace(/^[\s\r\n\t]+/, ""); // remove transport-padding CRLF
                // trimStart might not be available
                // Be careful! body-part allows trailing \r\n!
                // (as long as it is not part of `delimiter`)
                bodyParts = bodyPreambleTrimmed
                  .split(delimiter)
                  .map((s) => s.replace(/^[\s\r\n\t]+/, ""));
                // TODO: LWSP definition is actually trickier,
                // but should be fine in our case since without headers
                // we should just discard the part
              }
              for (const bodyPart of bodyParts) {
                const headers = new Headers();
                const headerOctetSeperatorIndex = bodyPart.indexOf("\r\n\r\n");
                if (headerOctetSeperatorIndex < 0) {
                  continue; // Skip unknown part
                }
                const headerText = bodyPart.slice(0, headerOctetSeperatorIndex);
                const octets = bodyPart.slice(headerOctetSeperatorIndex + 4);
                // TODO: use textproto.readMIMEHeader from deno_std
                const rawHeaders = headerText.split("\r\n");
                for (const rawHeader of rawHeaders) {
                  const sepIndex = rawHeader.indexOf(":");
                  if (sepIndex < 0) {
                    continue; // Skip this header
                  }
                  const key = rawHeader.slice(0, sepIndex);
                  const value = rawHeader.slice(sepIndex + 1);
                  headers.set(key, value);
                }
                if (!headers.has("content-disposition")) {
                  continue; // Skip unknown part
                }
                // Content-Transfer-Encoding Deprecated
                const contentDisposition = headers.get("content-disposition");
                const partContentType =
                  headers.get("content-type") || "text/plain";
                // TODO: custom charset encoding (needs TextEncoder support)
                // const contentTypeCharset =
                //   getHeaderValueParams(partContentType).get("charset") || "";
                if (!hasHeaderValueOf(contentDisposition, "form-data")) {
                  continue; // Skip, might not be form-data
                }
                const dispositionParams = getHeaderValueParams(
                  contentDisposition
                );
                if (!dispositionParams.has("name")) {
                  continue; // Skip, unknown name
                }
                const dispositionName = dispositionParams.get("name");
                if (dispositionParams.has("filename")) {
                  const filename = dispositionParams.get("filename");
                  const blob = new blob_ts_1.DenoBlob([enc.encode(octets)], {
                    type: partContentType,
                  });
                  // TODO: based on spec
                  // https://xhr.spec.whatwg.org/#dom-formdata-append
                  // https://xhr.spec.whatwg.org/#create-an-entry
                  // Currently it does not mention how I could pass content-type
                  // to the internally created file object...
                  formData.append(dispositionName, blob, filename);
                } else {
                  formData.append(dispositionName, octets);
                }
              }
              return formData;
            } else if (
              hasHeaderValueOf(
                this.contentType,
                "application/x-www-form-urlencoded"
              )
            ) {
              // From https://github.com/github/fetch/blob/master/fetch.js
              // Copyright (c) 2014-2016 GitHub, Inc. MIT License
              const body = await this.text();
              try {
                body
                  .trim()
                  .split("&")
                  .forEach((bytes) => {
                    if (bytes) {
                      const split = bytes.split("=");
                      const name = split.shift().replace(/\+/g, " ");
                      const value = split.join("=").replace(/\+/g, " ");
                      formData.append(
                        decodeURIComponent(name),
                        decodeURIComponent(value)
                      );
                    }
                  });
              } catch (e) {
                throw new TypeError("Invalid form urlencoded format");
              }
              return formData;
            } else {
              throw new TypeError("Invalid form data");
            }
          }
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          async json() {
            const text = await this.text();
            return JSON.parse(text);
          }
          async text() {
            const ab = await this.arrayBuffer();
            const decoder = new text_encoding_ts_9.TextDecoder("utf-8");
            return decoder.decode(ab);
          }
          read(p) {
            this.#bodyUsed = true;
            return io_ts_7.read(this.#rid, p);
          }
          close() {
            resources_ts_7.close(this.#rid);
            return Promise.resolve();
          }
          cancel() {
            return util_ts_24.notImplemented();
          }
          getReader() {
            return util_ts_24.notImplemented();
          }
          tee() {
            return util_ts_24.notImplemented();
          }
          [Symbol.asyncIterator]() {
            return io.toAsyncIterator(this);
          }
          get bodyUsed() {
            return this.#bodyUsed;
          }
          pipeThrough(_, _options) {
            return util_ts_24.notImplemented();
          }
          pipeTo(_dest, _options) {
            return util_ts_24.notImplemented();
          }
        };
        Response = class Response {
          constructor(
            url,
            status,
            statusText,
            headersList,
            rid,
            redirected_,
            type_ = "default",
            body_ = null
          ) {
            this.url = url;
            this.status = status;
            this.statusText = statusText;
            this.type_ = type_;
            this.#bodyViewable = () => {
              if (
                this.type == "error" ||
                this.type == "opaque" ||
                this.type == "opaqueredirect" ||
                this.body == undefined
              ) {
                return true;
              }
              return false;
            };
            this.trailer = util_ts_24.createResolvable();
            this.headers = new Headers(headersList);
            const contentType = this.headers.get("content-type") || "";
            if (body_ == null) {
              this.body = new Body(rid, contentType);
            } else {
              this.body = body_;
            }
            if (type_ == null) {
              this.type = "default";
            } else {
              this.type = type_;
              if (type_ == "error") {
                // spec: https://fetch.spec.whatwg.org/#concept-network-error
                this.status = 0;
                this.statusText = "";
                this.headers = new Headers();
                this.body = null;
                /* spec for other Response types:
                               https://fetch.spec.whatwg.org/#concept-filtered-response-basic
                               Please note that type "basic" is not the same thing as "default".*/
              } else if (type_ == "basic") {
                for (const h of this.headers) {
                  /* Forbidden Response-Header Names:
                                   https://fetch.spec.whatwg.org/#forbidden-response-header-name */
                  if (
                    ["set-cookie", "set-cookie2"].includes(h[0].toLowerCase())
                  ) {
                    this.headers.delete(h[0]);
                  }
                }
              } else if (type_ == "cors") {
                /* CORS-safelisted Response-Header Names:
                                 https://fetch.spec.whatwg.org/#cors-safelisted-response-header-name */
                const allowedHeaders = [
                  "Cache-Control",
                  "Content-Language",
                  "Content-Length",
                  "Content-Type",
                  "Expires",
                  "Last-Modified",
                  "Pragma",
                ].map((c) => c.toLowerCase());
                for (const h of this.headers) {
                  /* Technically this is still not standards compliant because we are
                                   supposed to allow headers allowed in the
                                   'Access-Control-Expose-Headers' header in the 'internal response'
                                   However, this implementation of response doesn't seem to have an
                                   easy way to access the internal response, so we ignore that
                                   header.
                                   TODO(serverhiccups): change how internal responses are handled
                                   so we can do this properly. */
                  if (!allowedHeaders.includes(h[0].toLowerCase())) {
                    this.headers.delete(h[0]);
                  }
                }
                /* TODO(serverhiccups): Once I fix the 'internal response' thing,
                               these actually need to treat the internal response differently */
              } else if (type_ == "opaque" || type_ == "opaqueredirect") {
                this.url = "";
                this.status = 0;
                this.statusText = "";
                this.headers = new Headers();
                this.body = null;
              }
            }
            this.redirected = redirected_;
          }
          #bodyViewable;
          arrayBuffer() {
            /* You have to do the null check here and not in the function because
             * otherwise TS complains about this.body potentially being null */
            if (this.#bodyViewable() || this.body == null) {
              return Promise.reject(new Error("Response body is null"));
            }
            return this.body.arrayBuffer();
          }
          blob() {
            if (this.#bodyViewable() || this.body == null) {
              return Promise.reject(new Error("Response body is null"));
            }
            return this.body.blob();
          }
          formData() {
            if (this.#bodyViewable() || this.body == null) {
              return Promise.reject(new Error("Response body is null"));
            }
            return this.body.formData();
          }
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          json() {
            if (this.#bodyViewable() || this.body == null) {
              return Promise.reject(new Error("Response body is null"));
            }
            return this.body.json();
          }
          text() {
            if (this.#bodyViewable() || this.body == null) {
              return Promise.reject(new Error("Response body is null"));
            }
            return this.body.text();
          }
          get ok() {
            return 200 <= this.status && this.status < 300;
          }
          get bodyUsed() {
            if (this.body === null) return false;
            return this.body.bodyUsed;
          }
          clone() {
            if (this.bodyUsed) {
              throw new TypeError(
                "Failed to execute 'clone' on 'Response': Response body is already used"
              );
            }
            const iterators = this.headers.entries();
            const headersList = [];
            for (const header of iterators) {
              headersList.push(header);
            }
            return new Response(
              this.url,
              this.status,
              this.statusText,
              headersList,
              -1,
              this.redirected,
              this.type,
              this.body
            );
          }
          static redirect(url, status) {
            if (![301, 302, 303, 307, 308].includes(status)) {
              throw new RangeError(
                "The redirection status must be one of 301, 302, 303, 307 and 308."
              );
            }
            return new Response(
              "",
              status,
              "",
              [["Location", typeof url === "string" ? url : url.toString()]],
              -1,
              false,
              "default",
              null
            );
          }
        };
        exports_101("Response", Response);
      },
    };
  }
);
System.register(
  "$deno$/web/headers.ts",
  ["$deno$/web/dom_iterable.ts", "$deno$/web/util.ts", "$deno$/web/console.ts"],
  function (exports_102, context_102) {
    "use strict";
    let dom_iterable_ts_2,
      util_ts_26,
      console_ts_4,
      invalidTokenRegex,
      invalidHeaderCharRegex,
      headerMap,
      HeadersBase,
      HeadersImpl;
    const __moduleName = context_102 && context_102.id;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    function isHeaders(value) {
      // eslint-disable-next-line @typescript-eslint/no-use-before-define
      return value instanceof Headers;
    }
    // TODO: headerGuard? Investigate if it is needed
    // node-fetch did not implement this but it is in the spec
    function normalizeParams(name, value) {
      name = String(name).toLowerCase();
      value = String(value).trim();
      return [name, value];
    }
    // The following name/value validations are copied from
    // https://github.com/bitinn/node-fetch/blob/master/src/headers.js
    // Copyright (c) 2016 David Frank. MIT License.
    function validateName(name) {
      if (invalidTokenRegex.test(name) || name === "") {
        throw new TypeError(`${name} is not a legal HTTP header name`);
      }
    }
    function validateValue(value) {
      if (invalidHeaderCharRegex.test(value)) {
        throw new TypeError(`${value} is not a legal HTTP header value`);
      }
    }
    return {
      setters: [
        function (dom_iterable_ts_2_1) {
          dom_iterable_ts_2 = dom_iterable_ts_2_1;
        },
        function (util_ts_26_1) {
          util_ts_26 = util_ts_26_1;
        },
        function (console_ts_4_1) {
          console_ts_4 = console_ts_4_1;
        },
      ],
      execute: function () {
        // From node-fetch
        // Copyright (c) 2016 David Frank. MIT License.
        invalidTokenRegex = /[^\^_`a-zA-Z\-0-9!#$%&'*+.|~]/;
        invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/;
        headerMap = Symbol("header map");
        // ref: https://fetch.spec.whatwg.org/#dom-headers
        HeadersBase = class HeadersBase {
          constructor(init) {
            if (init === null) {
              throw new TypeError(
                "Failed to construct 'Headers'; The provided value was not valid"
              );
            } else if (isHeaders(init)) {
              this[headerMap] = new Map(init);
            } else {
              this[headerMap] = new Map();
              if (Array.isArray(init)) {
                for (const tuple of init) {
                  // If header does not contain exactly two items,
                  // then throw a TypeError.
                  // ref: https://fetch.spec.whatwg.org/#concept-headers-fill
                  util_ts_26.requiredArguments(
                    "Headers.constructor tuple array argument",
                    tuple.length,
                    2
                  );
                  const [name, value] = normalizeParams(tuple[0], tuple[1]);
                  validateName(name);
                  validateValue(value);
                  const existingValue = this[headerMap].get(name);
                  this[headerMap].set(
                    name,
                    existingValue ? `${existingValue}, ${value}` : value
                  );
                }
              } else if (init) {
                const names = Object.keys(init);
                for (const rawName of names) {
                  const rawValue = init[rawName];
                  const [name, value] = normalizeParams(rawName, rawValue);
                  validateName(name);
                  validateValue(value);
                  this[headerMap].set(name, value);
                }
              }
            }
          }
          [console_ts_4.customInspect]() {
            let headerSize = this[headerMap].size;
            let output = "";
            this[headerMap].forEach((value, key) => {
              const prefix = headerSize === this[headerMap].size ? " " : "";
              const postfix = headerSize === 1 ? " " : ", ";
              output = output + `${prefix}${key}: ${value}${postfix}`;
              headerSize--;
            });
            return `Headers {${output}}`;
          }
          // ref: https://fetch.spec.whatwg.org/#concept-headers-append
          append(name, value) {
            util_ts_26.requiredArguments("Headers.append", arguments.length, 2);
            const [newname, newvalue] = normalizeParams(name, value);
            validateName(newname);
            validateValue(newvalue);
            const v = this[headerMap].get(newname);
            const str = v ? `${v}, ${newvalue}` : newvalue;
            this[headerMap].set(newname, str);
          }
          delete(name) {
            util_ts_26.requiredArguments("Headers.delete", arguments.length, 1);
            const [newname] = normalizeParams(name);
            validateName(newname);
            this[headerMap].delete(newname);
          }
          get(name) {
            util_ts_26.requiredArguments("Headers.get", arguments.length, 1);
            const [newname] = normalizeParams(name);
            validateName(newname);
            const value = this[headerMap].get(newname);
            return value || null;
          }
          has(name) {
            util_ts_26.requiredArguments("Headers.has", arguments.length, 1);
            const [newname] = normalizeParams(name);
            validateName(newname);
            return this[headerMap].has(newname);
          }
          set(name, value) {
            util_ts_26.requiredArguments("Headers.set", arguments.length, 2);
            const [newname, newvalue] = normalizeParams(name, value);
            validateName(newname);
            validateValue(newvalue);
            this[headerMap].set(newname, newvalue);
          }
          get [Symbol.toStringTag]() {
            return "Headers";
          }
        };
        // @internal
        HeadersImpl = class HeadersImpl extends dom_iterable_ts_2.DomIterableMixin(
          HeadersBase,
          headerMap
        ) {};
        exports_102("HeadersImpl", HeadersImpl);
      },
    };
  }
);
System.register(
  "$deno$/web/url_search_params.ts",
  ["$deno$/web/url.ts", "$deno$/web/util.ts"],
  function (exports_103, context_103) {
    "use strict";
    let url_ts_1, util_ts_27, urls, URLSearchParamsImpl;
    const __moduleName = context_103 && context_103.id;
    function handleStringInitialization(searchParams, init) {
      // Overload: USVString
      // If init is a string and starts with U+003F (?),
      // remove the first code point from init.
      if (init.charCodeAt(0) === 0x003f) {
        init = init.slice(1);
      }
      for (const pair of init.split("&")) {
        // Empty params are ignored
        if (pair.length === 0) {
          continue;
        }
        const position = pair.indexOf("=");
        const name = pair.slice(0, position === -1 ? pair.length : position);
        const value = pair.slice(name.length + 1);
        searchParams.append(
          decodeURIComponent(name),
          decodeURIComponent(value)
        );
      }
    }
    function handleArrayInitialization(searchParams, init) {
      // Overload: sequence<sequence<USVString>>
      for (const tuple of init) {
        // If pair does not contain exactly two items, then throw a TypeError.
        if (tuple.length !== 2) {
          throw new TypeError(
            "URLSearchParams.constructor tuple array argument must only contain pair elements"
          );
        }
        searchParams.append(tuple[0], tuple[1]);
      }
    }
    return {
      setters: [
        function (url_ts_1_1) {
          url_ts_1 = url_ts_1_1;
        },
        function (util_ts_27_1) {
          util_ts_27 = util_ts_27_1;
        },
      ],
      execute: function () {
        /** @internal */
        exports_103("urls", (urls = new WeakMap()));
        URLSearchParamsImpl = class URLSearchParamsImpl {
          constructor(init = "") {
            this.#params = [];
            this.#updateSteps = () => {
              const url = urls.get(this);
              if (url == null) {
                return;
              }
              let query = this.toString();
              if (query === "") {
                query = null;
              }
              url_ts_1.parts.get(url).query = query;
            };
            if (typeof init === "string") {
              handleStringInitialization(this, init);
              return;
            }
            if (Array.isArray(init) || util_ts_27.isIterable(init)) {
              handleArrayInitialization(this, init);
              return;
            }
            if (Object(init) !== init) {
              return;
            }
            if (init instanceof URLSearchParamsImpl) {
              this.#params = [...init.#params];
              return;
            }
            // Overload: record<USVString, USVString>
            for (const key of Object.keys(init)) {
              this.append(key, init[key]);
            }
            urls.set(this, null);
          }
          #params;
          #updateSteps;
          append(name, value) {
            util_ts_27.requiredArguments(
              "URLSearchParams.append",
              arguments.length,
              2
            );
            this.#params.push([String(name), String(value)]);
            this.#updateSteps();
          }
          delete(name) {
            util_ts_27.requiredArguments(
              "URLSearchParams.delete",
              arguments.length,
              1
            );
            name = String(name);
            let i = 0;
            while (i < this.#params.length) {
              if (this.#params[i][0] === name) {
                this.#params.splice(i, 1);
              } else {
                i++;
              }
            }
            this.#updateSteps();
          }
          getAll(name) {
            util_ts_27.requiredArguments(
              "URLSearchParams.getAll",
              arguments.length,
              1
            );
            name = String(name);
            const values = [];
            for (const entry of this.#params) {
              if (entry[0] === name) {
                values.push(entry[1]);
              }
            }
            return values;
          }
          get(name) {
            util_ts_27.requiredArguments(
              "URLSearchParams.get",
              arguments.length,
              1
            );
            name = String(name);
            for (const entry of this.#params) {
              if (entry[0] === name) {
                return entry[1];
              }
            }
            return null;
          }
          has(name) {
            util_ts_27.requiredArguments(
              "URLSearchParams.has",
              arguments.length,
              1
            );
            name = String(name);
            return this.#params.some((entry) => entry[0] === name);
          }
          set(name, value) {
            util_ts_27.requiredArguments(
              "URLSearchParams.set",
              arguments.length,
              2
            );
            // If there are any name-value pairs whose name is name, in list,
            // set the value of the first such name-value pair to value
            // and remove the others.
            name = String(name);
            value = String(value);
            let found = false;
            let i = 0;
            while (i < this.#params.length) {
              if (this.#params[i][0] === name) {
                if (!found) {
                  this.#params[i][1] = value;
                  found = true;
                  i++;
                } else {
                  this.#params.splice(i, 1);
                }
              } else {
                i++;
              }
            }
            // Otherwise, append a new name-value pair whose name is name
            // and value is value, to list.
            if (!found) {
              this.append(name, value);
            }
            this.#updateSteps();
          }
          sort() {
            this.#params.sort((a, b) =>
              a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1
            );
            this.#updateSteps();
          }
          forEach(
            callbackfn,
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            thisArg
          ) {
            util_ts_27.requiredArguments(
              "URLSearchParams.forEach",
              arguments.length,
              1
            );
            if (typeof thisArg !== "undefined") {
              callbackfn = callbackfn.bind(thisArg);
            }
            for (const [key, value] of this.entries()) {
              callbackfn(value, key, this);
            }
          }
          *keys() {
            for (const [key] of this.#params) {
              yield key;
            }
          }
          *values() {
            for (const [, value] of this.#params) {
              yield value;
            }
          }
          *entries() {
            yield* this.#params;
          }
          *[Symbol.iterator]() {
            yield* this.#params;
          }
          toString() {
            return this.#params
              .map(
                (tuple) =>
                  `${encodeURIComponent(tuple[0])}=${encodeURIComponent(
                    tuple[1]
                  )}`
              )
              .join("&");
          }
        };
        exports_103("URLSearchParamsImpl", URLSearchParamsImpl);
      },
    };
  }
);
System.register(
  "$deno$/ops/get_random_values.ts",
  ["$deno$/ops/dispatch_json.ts", "$deno$/util.ts"],
  function (exports_104, context_104) {
    "use strict";
    let dispatch_json_ts_36, util_ts_28;
    const __moduleName = context_104 && context_104.id;
    function getRandomValues(typedArray) {
      util_ts_28.assert(typedArray !== null, "Input must not be null");
      util_ts_28.assert(
        typedArray.length <= 65536,
        "Input must not be longer than 65536"
      );
      const ui8 = new Uint8Array(
        typedArray.buffer,
        typedArray.byteOffset,
        typedArray.byteLength
      );
      dispatch_json_ts_36.sendSync("op_get_random_values", {}, ui8);
      return typedArray;
    }
    exports_104("getRandomValues", getRandomValues);
    return {
      setters: [
        function (dispatch_json_ts_36_1) {
          dispatch_json_ts_36 = dispatch_json_ts_36_1;
        },
        function (util_ts_28_1) {
          util_ts_28 = util_ts_28_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/web/url.ts",
  [
    "$deno$/web/console.ts",
    "$deno$/web/url_search_params.ts",
    "$deno$/ops/get_random_values.ts",
  ],
  function (exports_105, context_105) {
    "use strict";
    let console_ts_5,
      url_search_params_ts_1,
      get_random_values_ts_1,
      patterns,
      urlRegExp,
      authorityRegExp,
      searchParamsMethods,
      blobURLMap,
      parts,
      URLImpl;
    const __moduleName = context_105 && context_105.id;
    function parse(url) {
      const urlMatch = urlRegExp.exec(url);
      if (urlMatch) {
        const [, , authority] = urlMatch;
        const authorityMatch = authority
          ? authorityRegExp.exec(authority)
          : [null, null, null, null, null];
        if (authorityMatch) {
          return {
            protocol: urlMatch[1] || "",
            username: authorityMatch[1] || "",
            password: authorityMatch[2] || "",
            hostname: authorityMatch[3] || "",
            port: authorityMatch[4] || "",
            path: urlMatch[3] || "",
            query: urlMatch[4] || "",
            hash: urlMatch[5] || "",
          };
        }
      }
      return undefined;
    }
    // Based on https://github.com/kelektiv/node-uuid
    // TODO(kevinkassimo): Use deno_std version once possible.
    function generateUUID() {
      return "00000000-0000-4000-8000-000000000000".replace(/[0]/g, () =>
        // random integer from 0 to 15 as a hex digit.
        (
          get_random_values_ts_1.getRandomValues(new Uint8Array(1))[0] % 16
        ).toString(16)
      );
    }
    function isAbsolutePath(path) {
      return path.startsWith("/");
    }
    // Resolves `.`s and `..`s where possible.
    // Preserves repeating and trailing `/`s by design.
    function normalizePath(path) {
      const isAbsolute = isAbsolutePath(path);
      path = path.replace(/^\//, "");
      const pathSegments = path.split("/");
      const newPathSegments = [];
      for (let i = 0; i < pathSegments.length; i++) {
        const previous = newPathSegments[newPathSegments.length - 1];
        if (
          pathSegments[i] == ".." &&
          previous != ".." &&
          (previous != undefined || isAbsolute)
        ) {
          newPathSegments.pop();
        } else if (pathSegments[i] != ".") {
          newPathSegments.push(pathSegments[i]);
        }
      }
      let newPath = newPathSegments.join("/");
      if (!isAbsolute) {
        if (newPathSegments.length == 0) {
          newPath = ".";
        }
      } else {
        newPath = `/${newPath}`;
      }
      return newPath;
    }
    // Standard URL basing logic, applied to paths.
    function resolvePathFromBase(path, basePath) {
      const normalizedPath = normalizePath(path);
      if (isAbsolutePath(normalizedPath)) {
        return normalizedPath;
      }
      const normalizedBasePath = normalizePath(basePath);
      if (!isAbsolutePath(normalizedBasePath)) {
        throw new TypeError("Base path must be absolute.");
      }
      // Special case.
      if (path == "") {
        return normalizedBasePath;
      }
      // Remove everything after the last `/` in `normalizedBasePath`.
      const prefix = normalizedBasePath.replace(/[^\/]*$/, "");
      // If `normalizedPath` ends with `.` or `..`, add a trailing space.
      const suffix = normalizedPath.replace(/(?<=(^|\/)(\.|\.\.))$/, "/");
      return normalizePath(prefix + suffix);
    }
    return {
      setters: [
        function (console_ts_5_1) {
          console_ts_5 = console_ts_5_1;
        },
        function (url_search_params_ts_1_1) {
          url_search_params_ts_1 = url_search_params_ts_1_1;
        },
        function (get_random_values_ts_1_1) {
          get_random_values_ts_1 = get_random_values_ts_1_1;
        },
      ],
      execute: function () {
        patterns = {
          protocol: "(?:([a-z]+):)",
          authority: "(?://([^/?#]*))",
          path: "([^?#]*)",
          query: "(\\?[^#]*)",
          hash: "(#.*)",
          authentication: "(?:([^:]*)(?::([^@]*))?@)",
          hostname: "([^:]+)",
          port: "(?::(\\d+))",
        };
        urlRegExp = new RegExp(
          `^${patterns.protocol}?${patterns.authority}?${patterns.path}${patterns.query}?${patterns.hash}?`
        );
        authorityRegExp = new RegExp(
          `^${patterns.authentication}?${patterns.hostname}${patterns.port}?$`
        );
        searchParamsMethods = ["append", "delete", "set"];
        // Keep it outside of URL to avoid any attempts of access.
        exports_105("blobURLMap", (blobURLMap = new Map()));
        /** @internal */
        exports_105("parts", (parts = new WeakMap()));
        URLImpl = class URLImpl {
          constructor(url, base) {
            this.#updateSearchParams = () => {
              const searchParams = new URLSearchParams(this.search);
              for (const methodName of searchParamsMethods) {
                /* eslint-disable @typescript-eslint/no-explicit-any */
                const method = searchParams[methodName];
                searchParams[methodName] = (...args) => {
                  method.apply(searchParams, args);
                  this.search = searchParams.toString();
                };
                /* eslint-enable */
              }
              this.#searchParams = searchParams;
              url_search_params_ts_1.urls.set(searchParams, this);
            };
            let baseParts;
            if (base) {
              baseParts =
                typeof base === "string" ? parse(base) : parts.get(base);
              if (!baseParts || baseParts.protocol == "") {
                throw new TypeError("Invalid base URL.");
              }
            }
            const urlParts = parse(url);
            if (!urlParts) {
              throw new TypeError("Invalid URL.");
            }
            if (urlParts.protocol) {
              parts.set(this, urlParts);
            } else if (baseParts) {
              parts.set(this, {
                protocol: baseParts.protocol,
                username: baseParts.username,
                password: baseParts.password,
                hostname: baseParts.hostname,
                port: baseParts.port,
                path: resolvePathFromBase(urlParts.path, baseParts.path || "/"),
                query: urlParts.query,
                hash: urlParts.hash,
              });
            } else {
              throw new TypeError("URL requires a base URL.");
            }
            this.#updateSearchParams();
          }
          #searchParams;
          [console_ts_5.customInspect]() {
            const keys = [
              "href",
              "origin",
              "protocol",
              "username",
              "password",
              "host",
              "hostname",
              "port",
              "pathname",
              "hash",
              "search",
            ];
            const objectString = keys
              .map((key) => `${key}: "${this[key] || ""}"`)
              .join(", ");
            return `URL { ${objectString} }`;
          }
          #updateSearchParams;
          get hash() {
            return parts.get(this).hash;
          }
          set hash(value) {
            value = unescape(String(value));
            if (!value) {
              parts.get(this).hash = "";
            } else {
              if (value.charAt(0) !== "#") {
                value = `#${value}`;
              }
              // hashes can contain % and # unescaped
              parts.get(this).hash = escape(value)
                .replace(/%25/g, "%")
                .replace(/%23/g, "#");
            }
          }
          get host() {
            return `${this.hostname}${this.port ? `:${this.port}` : ""}`;
          }
          set host(value) {
            value = String(value);
            const url = new URL(`http://${value}`);
            parts.get(this).hostname = url.hostname;
            parts.get(this).port = url.port;
          }
          get hostname() {
            return parts.get(this).hostname;
          }
          set hostname(value) {
            value = String(value);
            parts.get(this).hostname = encodeURIComponent(value);
          }
          get href() {
            const authentication =
              this.username || this.password
                ? `${this.username}${this.password ? ":" + this.password : ""}@`
                : "";
            let slash = "";
            if (this.host || this.protocol === "file:") {
              slash = "//";
            }
            return `${this.protocol}${slash}${authentication}${this.host}${this.pathname}${this.search}${this.hash}`;
          }
          set href(value) {
            value = String(value);
            if (value !== this.href) {
              const url = new URL(value);
              parts.set(this, { ...parts.get(url) });
              this.#updateSearchParams();
            }
          }
          get origin() {
            if (this.host) {
              return `${this.protocol}//${this.host}`;
            }
            return "null";
          }
          get password() {
            return parts.get(this).password;
          }
          set password(value) {
            value = String(value);
            parts.get(this).password = encodeURIComponent(value);
          }
          get pathname() {
            return parts.get(this)?.path || "/";
          }
          set pathname(value) {
            value = unescape(String(value));
            if (!value || value.charAt(0) !== "/") {
              value = `/${value}`;
            }
            // paths can contain % unescaped
            parts.get(this).path = escape(value).replace(/%25/g, "%");
          }
          get port() {
            return parts.get(this).port;
          }
          set port(value) {
            const port = parseInt(String(value), 10);
            parts.get(this).port = isNaN(port)
              ? ""
              : Math.max(0, port % 2 ** 16).toString();
          }
          get protocol() {
            return `${parts.get(this).protocol}:`;
          }
          set protocol(value) {
            value = String(value);
            if (value) {
              if (value.charAt(value.length - 1) === ":") {
                value = value.slice(0, -1);
              }
              parts.get(this).protocol = encodeURIComponent(value);
            }
          }
          get search() {
            const query = parts.get(this).query;
            if (query === null || query === "") {
              return "";
            }
            return query;
          }
          set search(value) {
            value = String(value);
            let query;
            if (value === "") {
              query = null;
            } else if (value.charAt(0) !== "?") {
              query = `?${value}`;
            } else {
              query = value;
            }
            parts.get(this).query = query;
            this.#updateSearchParams();
          }
          get username() {
            return parts.get(this).username;
          }
          set username(value) {
            value = String(value);
            parts.get(this).username = encodeURIComponent(value);
          }
          get searchParams() {
            return this.#searchParams;
          }
          toString() {
            return this.href;
          }
          toJSON() {
            return this.href;
          }
          // TODO(kevinkassimo): implement MediaSource version in the future.
          static createObjectURL(b) {
            const origin =
              globalThis.location.origin || "http://deno-opaque-origin";
            const key = `blob:${origin}/${generateUUID()}`;
            blobURLMap.set(key, b);
            return key;
          }
          static revokeObjectURL(url) {
            let urlObject;
            try {
              urlObject = new URL(url);
            } catch {
              throw new TypeError("Provided URL string is not valid");
            }
            if (urlObject.protocol !== "blob:") {
              return;
            }
            // Origin match check seems irrelevant for now, unless we implement
            // persisten storage for per globalThis.location.origin at some point.
            blobURLMap.delete(url);
          }
        };
        exports_105("URLImpl", URLImpl);
      },
    };
  }
);
System.register(
  "$deno$/ops/worker_host.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_106, context_106) {
    "use strict";
    let dispatch_json_ts_37;
    const __moduleName = context_106 && context_106.id;
    function createWorker(specifier, hasSourceCode, sourceCode, name) {
      return dispatch_json_ts_37.sendSync("op_create_worker", {
        specifier,
        hasSourceCode,
        sourceCode,
        name,
      });
    }
    exports_106("createWorker", createWorker);
    function hostTerminateWorker(id) {
      dispatch_json_ts_37.sendSync("op_host_terminate_worker", { id });
    }
    exports_106("hostTerminateWorker", hostTerminateWorker);
    function hostPostMessage(id, data) {
      dispatch_json_ts_37.sendSync("op_host_post_message", { id }, data);
    }
    exports_106("hostPostMessage", hostPostMessage);
    function hostGetMessage(id) {
      return dispatch_json_ts_37.sendAsync("op_host_get_message", { id });
    }
    exports_106("hostGetMessage", hostGetMessage);
    return {
      setters: [
        function (dispatch_json_ts_37_1) {
          dispatch_json_ts_37 = dispatch_json_ts_37_1;
        },
      ],
      execute: function () {},
    };
  }
);
System.register(
  "$deno$/web/workers.ts",
  [
    "$deno$/ops/worker_host.ts",
    "$deno$/util.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/web/event.ts",
    "$deno$/web/event_target.ts",
  ],
  function (exports_107, context_107) {
    "use strict";
    let worker_host_ts_1,
      util_ts_29,
      text_encoding_ts_10,
      event_ts_3,
      event_target_ts_1,
      encoder,
      decoder,
      MessageEvent,
      ErrorEvent,
      WorkerImpl;
    const __moduleName = context_107 && context_107.id;
    function encodeMessage(data) {
      const dataJson = JSON.stringify(data);
      return encoder.encode(dataJson);
    }
    function decodeMessage(dataIntArray) {
      const dataJson = decoder.decode(dataIntArray);
      return JSON.parse(dataJson);
    }
    return {
      setters: [
        function (worker_host_ts_1_1) {
          worker_host_ts_1 = worker_host_ts_1_1;
        },
        function (util_ts_29_1) {
          util_ts_29 = util_ts_29_1;
        },
        function (text_encoding_ts_10_1) {
          text_encoding_ts_10 = text_encoding_ts_10_1;
        },
        function (event_ts_3_1) {
          event_ts_3 = event_ts_3_1;
        },
        function (event_target_ts_1_1) {
          event_target_ts_1 = event_target_ts_1_1;
        },
      ],
      execute: function () {
        encoder = new text_encoding_ts_10.TextEncoder();
        decoder = new text_encoding_ts_10.TextDecoder();
        MessageEvent = class MessageEvent extends event_ts_3.EventImpl {
          constructor(type, eventInitDict) {
            super(type, {
              bubbles: eventInitDict?.bubbles ?? false,
              cancelable: eventInitDict?.cancelable ?? false,
              composed: eventInitDict?.composed ?? false,
            });
            this.data = eventInitDict?.data ?? null;
            this.origin = eventInitDict?.origin ?? "";
            this.lastEventId = eventInitDict?.lastEventId ?? "";
          }
        };
        exports_107("MessageEvent", MessageEvent);
        ErrorEvent = class ErrorEvent extends event_ts_3.EventImpl {
          constructor(type, eventInitDict) {
            super(type, {
              bubbles: eventInitDict?.bubbles ?? false,
              cancelable: eventInitDict?.cancelable ?? false,
              composed: eventInitDict?.composed ?? false,
            });
            this.message = eventInitDict?.message ?? "";
            this.filename = eventInitDict?.filename ?? "";
            this.lineno = eventInitDict?.lineno ?? 0;
            this.colno = eventInitDict?.colno ?? 0;
            this.error = eventInitDict?.error ?? null;
          }
        };
        exports_107("ErrorEvent", ErrorEvent);
        WorkerImpl = class WorkerImpl extends event_target_ts_1.EventTargetImpl {
          constructor(specifier, options) {
            super();
            this.#terminated = false;
            this.#handleMessage = (msgData) => {
              let data;
              try {
                data = decodeMessage(new Uint8Array(msgData));
              } catch (e) {
                const msgErrorEvent = new MessageEvent("messageerror", {
                  cancelable: false,
                  data,
                });
                if (this.onmessageerror) {
                  this.onmessageerror(msgErrorEvent);
                }
                return;
              }
              const msgEvent = new MessageEvent("message", {
                cancelable: false,
                data,
              });
              if (this.onmessage) {
                this.onmessage(msgEvent);
              }
              this.dispatchEvent(msgEvent);
            };
            this.#handleError = (e) => {
              const event = new ErrorEvent("error", {
                cancelable: true,
                message: e.message,
                lineno: e.lineNumber ? e.lineNumber + 1 : undefined,
                colno: e.columnNumber ? e.columnNumber + 1 : undefined,
                filename: e.fileName,
                error: null,
              });
              let handled = false;
              if (this.onerror) {
                this.onerror(event);
              }
              this.dispatchEvent(event);
              if (event.defaultPrevented) {
                handled = true;
              }
              return handled;
            };
            this.#poll = async () => {
              while (!this.#terminated) {
                const event = await worker_host_ts_1.hostGetMessage(this.#id);
                // If terminate was called then we ignore all messages
                if (this.#terminated) {
                  return;
                }
                const type = event.type;
                if (type === "terminalError") {
                  this.#terminated = true;
                  if (!this.#handleError(event.error)) {
                    throw Error(event.error.message);
                  }
                  continue;
                }
                if (type === "msg") {
                  this.#handleMessage(event.data);
                  continue;
                }
                if (type === "error") {
                  if (!this.#handleError(event.error)) {
                    throw Error(event.error.message);
                  }
                  continue;
                }
                if (type === "close") {
                  util_ts_29.log(
                    `Host got "close" message from worker: ${this.#name}`
                  );
                  this.#terminated = true;
                  return;
                }
                throw new Error(`Unknown worker event: "${type}"`);
              }
            };
            const { type = "classic", name = "unknown" } = options ?? {};
            if (type !== "module") {
              throw new Error(
                'Not yet implemented: only "module" type workers are supported'
              );
            }
            this.#name = name;
            const hasSourceCode = false;
            const sourceCode = decoder.decode(new Uint8Array());
            /* TODO(bartlomieju):
                    // Handle blob URL.
                    if (specifier.startsWith("blob:")) {
                      hasSourceCode = true;
                      const b = blobURLMap.get(specifier);
                      if (!b) {
                        throw new Error("No Blob associated with the given URL is found");
                      }
                      const blobBytes = blobBytesWeakMap.get(b!);
                      if (!blobBytes) {
                        throw new Error("Invalid Blob");
                      }
                      sourceCode = blobBytes!;
                    }
                    */
            const { id } = worker_host_ts_1.createWorker(
              specifier,
              hasSourceCode,
              sourceCode,
              options?.name
            );
            this.#id = id;
            this.#poll();
          }
          #id;
          #name;
          #terminated;
          #handleMessage;
          #handleError;
          #poll;
          postMessage(message, transferOrOptions) {
            if (transferOrOptions) {
              throw new Error(
                "Not yet implemented: `transfer` and `options` are not supported."
              );
            }
            if (this.#terminated) {
              return;
            }
            worker_host_ts_1.hostPostMessage(this.#id, encodeMessage(message));
          }
          terminate() {
            if (!this.#terminated) {
              this.#terminated = true;
              worker_host_ts_1.hostTerminateWorker(this.#id);
            }
          }
        };
        exports_107("WorkerImpl", WorkerImpl);
      },
    };
  }
);
System.register(
  "$deno$/web/performance.ts",
  ["$deno$/ops/timers.ts"],
  function (exports_108, context_108) {
    "use strict";
    let timers_ts_3, Performance;
    const __moduleName = context_108 && context_108.id;
    return {
      setters: [
        function (timers_ts_3_1) {
          timers_ts_3 = timers_ts_3_1;
        },
      ],
      execute: function () {
        Performance = class Performance {
          now() {
            const res = timers_ts_3.now();
            return res.seconds * 1e3 + res.subsecNanos / 1e6;
          }
        };
        exports_108("Performance", Performance);
      },
    };
  }
);
System.register(
  "$deno$/web/body.ts",
  [
    "$deno$/web/blob.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/web/streams/mod.ts",
  ],
  function (exports_109, context_109) {
    "use strict";
    let blob, encoding, mod_ts_2, TextEncoder, TextDecoder, DenoBlob, Body;
    const __moduleName = context_109 && context_109.id;
    function validateBodyType(owner, bodySource) {
      if (
        bodySource instanceof Int8Array ||
        bodySource instanceof Int16Array ||
        bodySource instanceof Int32Array ||
        bodySource instanceof Uint8Array ||
        bodySource instanceof Uint16Array ||
        bodySource instanceof Uint32Array ||
        bodySource instanceof Uint8ClampedArray ||
        bodySource instanceof Float32Array ||
        bodySource instanceof Float64Array
      ) {
        return true;
      } else if (bodySource instanceof ArrayBuffer) {
        return true;
      } else if (typeof bodySource === "string") {
        return true;
      } else if (bodySource instanceof mod_ts_2.ReadableStream) {
        return true;
      } else if (bodySource instanceof FormData) {
        return true;
      } else if (!bodySource) {
        return true; // null body is fine
      }
      throw new Error(
        `Bad ${owner.constructor.name} body type: ${bodySource.constructor.name}`
      );
    }
    function concatenate(...arrays) {
      let totalLength = 0;
      for (const arr of arrays) {
        totalLength += arr.length;
      }
      const result = new Uint8Array(totalLength);
      let offset = 0;
      for (const arr of arrays) {
        result.set(arr, offset);
        offset += arr.length;
      }
      return result.buffer;
    }
    function bufferFromStream(stream) {
      return new Promise((resolve, reject) => {
        const parts = [];
        const encoder = new TextEncoder();
        // recurse
        (function pump() {
          stream
            .read()
            .then(({ done, value }) => {
              if (done) {
                return resolve(concatenate(...parts));
              }
              if (typeof value === "string") {
                parts.push(encoder.encode(value));
              } else if (value instanceof ArrayBuffer) {
                parts.push(new Uint8Array(value));
              } else if (!value) {
                // noop for undefined
              } else {
                reject("unhandled type on stream read");
              }
              return pump();
            })
            .catch((err) => {
              reject(err);
            });
        })();
      });
    }
    function getHeaderValueParams(value) {
      const params = new Map();
      // Forced to do so for some Map constructor param mismatch
      value
        .split(";")
        .slice(1)
        .map((s) => s.trim().split("="))
        .filter((arr) => arr.length > 1)
        .map(([k, v]) => [k, v.replace(/^"([^"]*)"$/, "$1")])
        .forEach(([k, v]) => params.set(k, v));
      return params;
    }
    function hasHeaderValueOf(s, value) {
      return new RegExp(`^${value}[\t\s]*;?`).test(s);
    }
    return {
      setters: [
        function (blob_3) {
          blob = blob_3;
        },
        function (encoding_1) {
          encoding = encoding_1;
        },
        function (mod_ts_2_1) {
          mod_ts_2 = mod_ts_2_1;
        },
      ],
      execute: function () {
        // only namespace imports work for now, plucking out what we need
        (TextEncoder = encoding.TextEncoder),
          (TextDecoder = encoding.TextDecoder);
        DenoBlob = blob.DenoBlob;
        exports_109(
          "BodyUsedError",
          "Failed to execute 'clone' on 'Body': body is already used"
        );
        Body = class Body {
          constructor(_bodySource, contentType) {
            this._bodySource = _bodySource;
            this.contentType = contentType;
            validateBodyType(this, _bodySource);
            this._bodySource = _bodySource;
            this.contentType = contentType;
            this._stream = null;
          }
          get body() {
            if (this._stream) {
              return this._stream;
            }
            if (this._bodySource instanceof mod_ts_2.ReadableStream) {
              // @ts-ignore
              this._stream = this._bodySource;
            }
            if (typeof this._bodySource === "string") {
              const bodySource = this._bodySource;
              this._stream = new mod_ts_2.ReadableStream({
                start(controller) {
                  controller.enqueue(bodySource);
                  controller.close();
                },
              });
            }
            return this._stream;
          }
          get bodyUsed() {
            if (this.body && this.body.locked) {
              return true;
            }
            return false;
          }
          async blob() {
            return new DenoBlob([await this.arrayBuffer()]);
          }
          // ref: https://fetch.spec.whatwg.org/#body-mixin
          async formData() {
            const formData = new FormData();
            const enc = new TextEncoder();
            if (hasHeaderValueOf(this.contentType, "multipart/form-data")) {
              const params = getHeaderValueParams(this.contentType);
              if (!params.has("boundary")) {
                // TypeError is required by spec
                throw new TypeError(
                  "multipart/form-data must provide a boundary"
                );
              }
              // ref: https://tools.ietf.org/html/rfc2046#section-5.1
              const boundary = params.get("boundary");
              const dashBoundary = `--${boundary}`;
              const delimiter = `\r\n${dashBoundary}`;
              const closeDelimiter = `${delimiter}--`;
              const body = await this.text();
              let bodyParts;
              const bodyEpilogueSplit = body.split(closeDelimiter);
              if (bodyEpilogueSplit.length < 2) {
                bodyParts = [];
              } else {
                // discard epilogue
                const bodyEpilogueTrimmed = bodyEpilogueSplit[0];
                // first boundary treated special due to optional prefixed \r\n
                const firstBoundaryIndex = bodyEpilogueTrimmed.indexOf(
                  dashBoundary
                );
                if (firstBoundaryIndex < 0) {
                  throw new TypeError("Invalid boundary");
                }
                const bodyPreambleTrimmed = bodyEpilogueTrimmed
                  .slice(firstBoundaryIndex + dashBoundary.length)
                  .replace(/^[\s\r\n\t]+/, ""); // remove transport-padding CRLF
                // trimStart might not be available
                // Be careful! body-part allows trailing \r\n!
                // (as long as it is not part of `delimiter`)
                bodyParts = bodyPreambleTrimmed
                  .split(delimiter)
                  .map((s) => s.replace(/^[\s\r\n\t]+/, ""));
                // TODO: LWSP definition is actually trickier,
                // but should be fine in our case since without headers
                // we should just discard the part
              }
              for (const bodyPart of bodyParts) {
                const headers = new Headers();
                const headerOctetSeperatorIndex = bodyPart.indexOf("\r\n\r\n");
                if (headerOctetSeperatorIndex < 0) {
                  continue; // Skip unknown part
                }
                const headerText = bodyPart.slice(0, headerOctetSeperatorIndex);
                const octets = bodyPart.slice(headerOctetSeperatorIndex + 4);
                // TODO: use textproto.readMIMEHeader from deno_std
                const rawHeaders = headerText.split("\r\n");
                for (const rawHeader of rawHeaders) {
                  const sepIndex = rawHeader.indexOf(":");
                  if (sepIndex < 0) {
                    continue; // Skip this header
                  }
                  const key = rawHeader.slice(0, sepIndex);
                  const value = rawHeader.slice(sepIndex + 1);
                  headers.set(key, value);
                }
                if (!headers.has("content-disposition")) {
                  continue; // Skip unknown part
                }
                // Content-Transfer-Encoding Deprecated
                const contentDisposition = headers.get("content-disposition");
                const partContentType =
                  headers.get("content-type") || "text/plain";
                // TODO: custom charset encoding (needs TextEncoder support)
                // const contentTypeCharset =
                //   getHeaderValueParams(partContentType).get("charset") || "";
                if (!hasHeaderValueOf(contentDisposition, "form-data")) {
                  continue; // Skip, might not be form-data
                }
                const dispositionParams = getHeaderValueParams(
                  contentDisposition
                );
                if (!dispositionParams.has("name")) {
                  continue; // Skip, unknown name
                }
                const dispositionName = dispositionParams.get("name");
                if (dispositionParams.has("filename")) {
                  const filename = dispositionParams.get("filename");
                  const blob = new DenoBlob([enc.encode(octets)], {
                    type: partContentType,
                  });
                  // TODO: based on spec
                  // https://xhr.spec.whatwg.org/#dom-formdata-append
                  // https://xhr.spec.whatwg.org/#create-an-entry
                  // Currently it does not mention how I could pass content-type
                  // to the internally created file object...
                  formData.append(dispositionName, blob, filename);
                } else {
                  formData.append(dispositionName, octets);
                }
              }
              return formData;
            } else if (
              hasHeaderValueOf(
                this.contentType,
                "application/x-www-form-urlencoded"
              )
            ) {
              // From https://github.com/github/fetch/blob/master/fetch.js
              // Copyright (c) 2014-2016 GitHub, Inc. MIT License
              const body = await this.text();
              try {
                body
                  .trim()
                  .split("&")
                  .forEach((bytes) => {
                    if (bytes) {
                      const split = bytes.split("=");
                      const name = split.shift().replace(/\+/g, " ");
                      const value = split.join("=").replace(/\+/g, " ");
                      formData.append(
                        decodeURIComponent(name),
                        decodeURIComponent(value)
                      );
                    }
                  });
              } catch (e) {
                throw new TypeError("Invalid form urlencoded format");
              }
              return formData;
            } else {
              throw new TypeError("Invalid form data");
            }
          }
          async text() {
            if (typeof this._bodySource === "string") {
              return this._bodySource;
            }
            const ab = await this.arrayBuffer();
            const decoder = new TextDecoder("utf-8");
            return decoder.decode(ab);
          }
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          async json() {
            const raw = await this.text();
            return JSON.parse(raw);
          }
          arrayBuffer() {
            if (
              this._bodySource instanceof Int8Array ||
              this._bodySource instanceof Int16Array ||
              this._bodySource instanceof Int32Array ||
              this._bodySource instanceof Uint8Array ||
              this._bodySource instanceof Uint16Array ||
              this._bodySource instanceof Uint32Array ||
              this._bodySource instanceof Uint8ClampedArray ||
              this._bodySource instanceof Float32Array ||
              this._bodySource instanceof Float64Array
            ) {
              return Promise.resolve(this._bodySource.buffer);
            } else if (this._bodySource instanceof ArrayBuffer) {
              return Promise.resolve(this._bodySource);
            } else if (typeof this._bodySource === "string") {
              const enc = new TextEncoder();
              return Promise.resolve(enc.encode(this._bodySource).buffer);
            } else if (this._bodySource instanceof mod_ts_2.ReadableStream) {
              // @ts-ignore
              return bufferFromStream(this._bodySource.getReader());
            } else if (this._bodySource instanceof FormData) {
              const enc = new TextEncoder();
              return Promise.resolve(
                enc.encode(this._bodySource.toString()).buffer
              );
            } else if (!this._bodySource) {
              return Promise.resolve(new ArrayBuffer(0));
            }
            throw new Error(
              `Body type not yet implemented: ${this._bodySource.constructor.name}`
            );
          }
        };
        exports_109("Body", Body);
      },
    };
  }
);
System.register(
  "$deno$/web/request.ts",
  ["$deno$/web/body.ts", "$deno$/web/streams/mod.ts"],
  function (exports_110, context_110) {
    "use strict";
    let body, streams, ReadableStream, Request;
    const __moduleName = context_110 && context_110.id;
    function byteUpperCase(s) {
      return String(s).replace(/[a-z]/g, function byteUpperCaseReplace(c) {
        return c.toUpperCase();
      });
    }
    function normalizeMethod(m) {
      const u = byteUpperCase(m);
      if (
        u === "DELETE" ||
        u === "GET" ||
        u === "HEAD" ||
        u === "OPTIONS" ||
        u === "POST" ||
        u === "PUT"
      ) {
        return u;
      }
      return m;
    }
    return {
      setters: [
        function (body_1) {
          body = body_1;
        },
        function (streams_1) {
          streams = streams_1;
        },
      ],
      execute: function () {
        ReadableStream = streams.ReadableStream;
        Request = class Request extends body.Body {
          constructor(input, init) {
            if (arguments.length < 1) {
              throw TypeError("Not enough arguments");
            }
            if (!init) {
              init = {};
            }
            let b;
            // prefer body from init
            if (init.body) {
              b = init.body;
            } else if (input instanceof Request && input._bodySource) {
              if (input.bodyUsed) {
                throw TypeError(body.BodyUsedError);
              }
              b = input._bodySource;
            } else if (
              typeof input === "object" &&
              "body" in input &&
              input.body
            ) {
              if (input.bodyUsed) {
                throw TypeError(body.BodyUsedError);
              }
              b = input.body;
            } else {
              b = "";
            }
            let headers;
            // prefer headers from init
            if (init.headers) {
              headers = new Headers(init.headers);
            } else if (input instanceof Request) {
              headers = input.headers;
            } else {
              headers = new Headers();
            }
            const contentType = headers.get("content-type") || "";
            super(b, contentType);
            this.headers = headers;
            // readonly attribute ByteString method;
            this.method = "GET";
            // readonly attribute USVString url;
            this.url = "";
            // readonly attribute RequestCredentials credentials;
            this.credentials = "omit";
            if (input instanceof Request) {
              if (input.bodyUsed) {
                throw TypeError(body.BodyUsedError);
              }
              this.method = input.method;
              this.url = input.url;
              this.headers = new Headers(input.headers);
              this.credentials = input.credentials;
              this._stream = input._stream;
            } else if (typeof input === "string") {
              this.url = input;
            }
            if (init && "method" in init) {
              this.method = normalizeMethod(init.method);
            }
            if (
              init &&
              "credentials" in init &&
              init.credentials &&
              ["omit", "same-origin", "include"].indexOf(init.credentials) !==
                -1
            ) {
              this.credentials = init.credentials;
            }
          }
          clone() {
            if (this.bodyUsed) {
              throw TypeError(body.BodyUsedError);
            }
            const iterators = this.headers.entries();
            const headersList = [];
            for (const header of iterators) {
              headersList.push(header);
            }
            let body2 = this._bodySource;
            if (this._bodySource instanceof ReadableStream) {
              const tees = this._bodySource.tee();
              this._stream = this._bodySource = tees[0];
              body2 = tees[1];
            }
            const cloned = new Request(this.url, {
              body: body2,
              method: this.method,
              headers: new Headers(headersList),
              credentials: this.credentials,
            });
            return cloned;
          }
        };
        exports_110("Request", Request);
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/globals.ts",
  [
    "./lib.deno.shared_globals.d.ts",
    "$deno$/web/blob.ts",
    "$deno$/web/console.ts",
    "$deno$/web/custom_event.ts",
    "$deno$/web/dom_exception.ts",
    "$deno$/web/dom_file.ts",
    "$deno$/web/event.ts",
    "$deno$/web/event_target.ts",
    "$deno$/web/form_data.ts",
    "$deno$/web/fetch.ts",
    "$deno$/web/headers.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/web/timers.ts",
    "$deno$/web/url.ts",
    "$deno$/web/url_search_params.ts",
    "$deno$/web/workers.ts",
    "$deno$/web/performance.ts",
    "$deno$/web/request.ts",
    "$deno$/web/streams/mod.ts",
    "$deno$/core.ts",
  ],
  function (exports_111, context_111) {
    "use strict";
    let blob,
      consoleTypes,
      customEvent,
      domException,
      domFile,
      event,
      eventTarget,
      formData,
      fetchTypes,
      headers,
      textEncoding,
      timers,
      url,
      urlSearchParams,
      workers,
      performanceUtil,
      request,
      streams,
      core_ts_8;
    const __moduleName = context_111 && context_111.id;
    function writable(value) {
      return {
        value,
        writable: true,
        enumerable: true,
        configurable: true,
      };
    }
    exports_111("writable", writable);
    function nonEnumerable(value) {
      return {
        value,
        writable: true,
        configurable: true,
      };
    }
    exports_111("nonEnumerable", nonEnumerable);
    function readOnly(value) {
      return {
        value,
        enumerable: true,
      };
    }
    exports_111("readOnly", readOnly);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    function getterOnly(getter) {
      return {
        get: getter,
        enumerable: true,
      };
    }
    exports_111("getterOnly", getterOnly);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    function setEventTargetData(value) {
      eventTarget.eventTargetData.set(
        value,
        eventTarget.getDefaultTargetData()
      );
    }
    exports_111("setEventTargetData", setEventTargetData);
    return {
      setters: [
        function (_1) {},
        function (blob_4) {
          blob = blob_4;
        },
        function (consoleTypes_1) {
          consoleTypes = consoleTypes_1;
        },
        function (customEvent_1) {
          customEvent = customEvent_1;
        },
        function (domException_1) {
          domException = domException_1;
        },
        function (domFile_2) {
          domFile = domFile_2;
        },
        function (event_1) {
          event = event_1;
        },
        function (eventTarget_1) {
          eventTarget = eventTarget_1;
        },
        function (formData_1) {
          formData = formData_1;
        },
        function (fetchTypes_1) {
          fetchTypes = fetchTypes_1;
        },
        function (headers_1) {
          headers = headers_1;
        },
        function (textEncoding_1) {
          textEncoding = textEncoding_1;
        },
        function (timers_1) {
          timers = timers_1;
        },
        function (url_1) {
          url = url_1;
        },
        function (urlSearchParams_1) {
          urlSearchParams = urlSearchParams_1;
        },
        function (workers_1) {
          workers = workers_1;
        },
        function (performanceUtil_1) {
          performanceUtil = performanceUtil_1;
        },
        function (request_1) {
          request = request_1;
        },
        function (streams_2) {
          streams = streams_2;
        },
        function (core_ts_8_1) {
          core_ts_8 = core_ts_8_1;
        },
      ],
      execute: function () {
        // https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
        exports_111("windowOrWorkerGlobalScopeMethods", {
          atob: writable(textEncoding.atob),
          btoa: writable(textEncoding.btoa),
          clearInterval: writable(timers.clearInterval),
          clearTimeout: writable(timers.clearTimeout),
          fetch: writable(fetchTypes.fetch),
          // queueMicrotask is bound in Rust
          setInterval: writable(timers.setInterval),
          setTimeout: writable(timers.setTimeout),
        });
        // Other properties shared between WindowScope and WorkerGlobalScope
        exports_111("windowOrWorkerGlobalScopeProperties", {
          console: writable(new consoleTypes.Console(core_ts_8.core.print)),
          Blob: nonEnumerable(blob.DenoBlob),
          File: nonEnumerable(domFile.DomFileImpl),
          CustomEvent: nonEnumerable(customEvent.CustomEventImpl),
          DOMException: nonEnumerable(domException.DOMExceptionImpl),
          Event: nonEnumerable(event.EventImpl),
          EventTarget: nonEnumerable(eventTarget.EventTargetImpl),
          URL: nonEnumerable(url.URLImpl),
          URLSearchParams: nonEnumerable(urlSearchParams.URLSearchParamsImpl),
          Headers: nonEnumerable(headers.HeadersImpl),
          FormData: nonEnumerable(formData.FormDataImpl),
          TextEncoder: nonEnumerable(textEncoding.TextEncoder),
          TextDecoder: nonEnumerable(textEncoding.TextDecoder),
          ReadableStream: nonEnumerable(streams.ReadableStream),
          Request: nonEnumerable(request.Request),
          Response: nonEnumerable(fetchTypes.Response),
          performance: writable(new performanceUtil.Performance()),
          Worker: nonEnumerable(workers.WorkerImpl),
        });
        exports_111("eventTargetProperties", {
          addEventListener: readOnly(
            eventTarget.EventTargetImpl.prototype.addEventListener
          ),
          dispatchEvent: readOnly(
            eventTarget.EventTargetImpl.prototype.dispatchEvent
          ),
          removeEventListener: readOnly(
            eventTarget.EventTargetImpl.prototype.removeEventListener
          ),
        });
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/ops/web_worker.ts",
  ["$deno$/ops/dispatch_json.ts"],
  function (exports_112, context_112) {
    "use strict";
    let dispatch_json_ts_38;
    const __moduleName = context_112 && context_112.id;
    function postMessage(data) {
      dispatch_json_ts_38.sendSync("op_worker_post_message", {}, data);
    }
    exports_112("postMessage", postMessage);
    function close() {
      dispatch_json_ts_38.sendSync("op_worker_close");
    }
    exports_112("close", close);
    return {
      setters: [
        function (dispatch_json_ts_38_1) {
          dispatch_json_ts_38 = dispatch_json_ts_38_1;
        },
      ],
      execute: function () {},
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/web/dom_util.ts", [], function (
  exports_113,
  context_113
) {
  "use strict";
  const __moduleName = context_113 && context_113.id;
  function getDOMStringList(arr) {
    Object.defineProperties(arr, {
      contains: {
        value(searchElement) {
          return arr.includes(searchElement);
        },
        enumerable: true,
      },
      item: {
        value(idx) {
          return idx in arr ? arr[idx] : null;
        },
      },
    });
    return arr;
  }
  exports_113("getDOMStringList", getDOMStringList);
  return {
    setters: [],
    execute: function () {},
  };
});
System.register(
  "$deno$/web/location.ts",
  ["$deno$/util.ts", "$deno$/web/dom_util.ts"],
  function (exports_114, context_114) {
    "use strict";
    let util_ts_30, dom_util_ts_1, LocationImpl;
    const __moduleName = context_114 && context_114.id;
    /** Sets the `window.location` at runtime.
     * @internal */
    function setLocation(url) {
      globalThis.location = new LocationImpl(url);
      Object.freeze(globalThis.location);
    }
    exports_114("setLocation", setLocation);
    return {
      setters: [
        function (util_ts_30_1) {
          util_ts_30 = util_ts_30_1;
        },
        function (dom_util_ts_1_1) {
          dom_util_ts_1 = dom_util_ts_1_1;
        },
      ],
      execute: function () {
        LocationImpl = class LocationImpl {
          constructor(url) {
            this.ancestorOrigins = dom_util_ts_1.getDOMStringList([]);
            const u = new URL(url);
            this.#url = u;
            this.hash = u.hash;
            this.host = u.host;
            this.href = u.href;
            this.hostname = u.hostname;
            this.origin = u.protocol + "//" + u.host;
            this.pathname = u.pathname;
            this.protocol = u.protocol;
            this.port = u.port;
            this.search = u.search;
          }
          #url;
          toString() {
            return this.#url.toString();
          }
          assign(_url) {
            throw util_ts_30.notImplemented();
          }
          reload() {
            throw util_ts_30.notImplemented();
          }
          replace(_url) {
            throw util_ts_30.notImplemented();
          }
        };
        exports_114("LocationImpl", LocationImpl);
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/runtime_worker.ts",
  [
    "$deno$/globals.ts",
    "$deno$/ops/web_worker.ts",
    "$deno$/web/location.ts",
    "$deno$/util.ts",
    "$deno$/web/workers.ts",
    "$deno$/web/text_encoding.ts",
    "$deno$/runtime.ts",
  ],
  function (exports_115, context_115) {
    "use strict";
    let globals_ts_1,
      webWorkerOps,
      location_ts_1,
      util_ts_31,
      workers_ts_1,
      text_encoding_ts_11,
      runtime,
      encoder,
      onmessage,
      onerror,
      isClosing,
      hasBootstrapped,
      workerRuntimeGlobalProperties;
    const __moduleName = context_115 && context_115.id;
    function postMessage(data) {
      const dataJson = JSON.stringify(data);
      const dataIntArray = encoder.encode(dataJson);
      webWorkerOps.postMessage(dataIntArray);
    }
    exports_115("postMessage", postMessage);
    function close() {
      if (isClosing) {
        return;
      }
      isClosing = true;
      webWorkerOps.close();
    }
    exports_115("close", close);
    async function workerMessageRecvCallback(data) {
      const msgEvent = new workers_ts_1.MessageEvent("message", {
        cancelable: false,
        data,
      });
      try {
        if (globalThis["onmessage"]) {
          const result = globalThis.onmessage(msgEvent);
          if (result && "then" in result) {
            await result;
          }
        }
        globalThis.dispatchEvent(msgEvent);
      } catch (e) {
        let handled = false;
        const errorEvent = new workers_ts_1.ErrorEvent("error", {
          cancelable: true,
          message: e.message,
          lineno: e.lineNumber ? e.lineNumber + 1 : undefined,
          colno: e.columnNumber ? e.columnNumber + 1 : undefined,
          filename: e.fileName,
          error: null,
        });
        if (globalThis["onerror"]) {
          const ret = globalThis.onerror(
            e.message,
            e.fileName,
            e.lineNumber,
            e.columnNumber,
            e
          );
          handled = ret === true;
        }
        globalThis.dispatchEvent(errorEvent);
        if (errorEvent.defaultPrevented) {
          handled = true;
        }
        if (!handled) {
          throw e;
        }
      }
    }
    exports_115("workerMessageRecvCallback", workerMessageRecvCallback);
    function bootstrapWorkerRuntime(name, internalName) {
      if (hasBootstrapped) {
        throw new Error("Worker runtime already bootstrapped");
      }
      util_ts_31.log("bootstrapWorkerRuntime");
      hasBootstrapped = true;
      Object.defineProperties(
        globalThis,
        globals_ts_1.windowOrWorkerGlobalScopeMethods
      );
      Object.defineProperties(
        globalThis,
        globals_ts_1.windowOrWorkerGlobalScopeProperties
      );
      Object.defineProperties(globalThis, workerRuntimeGlobalProperties);
      Object.defineProperties(globalThis, globals_ts_1.eventTargetProperties);
      Object.defineProperties(globalThis, {
        name: globals_ts_1.readOnly(name),
      });
      globals_ts_1.setEventTargetData(globalThis);
      const s = runtime.start(internalName ?? name);
      const location = new location_ts_1.LocationImpl(s.location);
      util_ts_31.immutableDefine(globalThis, "location", location);
      Object.freeze(globalThis.location);
      // globalThis.Deno is not available in worker scope
      delete globalThis.Deno;
      util_ts_31.assert(globalThis.Deno === undefined);
    }
    exports_115("bootstrapWorkerRuntime", bootstrapWorkerRuntime);
    return {
      setters: [
        function (globals_ts_1_1) {
          globals_ts_1 = globals_ts_1_1;
        },
        function (webWorkerOps_1) {
          webWorkerOps = webWorkerOps_1;
        },
        function (location_ts_1_1) {
          location_ts_1 = location_ts_1_1;
        },
        function (util_ts_31_1) {
          util_ts_31 = util_ts_31_1;
        },
        function (workers_ts_1_1) {
          workers_ts_1 = workers_ts_1_1;
        },
        function (text_encoding_ts_11_1) {
          text_encoding_ts_11 = text_encoding_ts_11_1;
        },
        function (runtime_1) {
          runtime = runtime_1;
        },
      ],
      execute: function () {
        encoder = new text_encoding_ts_11.TextEncoder();
        // TODO(bartlomieju): remove these funtions
        // Stuff for workers
        exports_115("onmessage", (onmessage = () => {}));
        exports_115("onerror", (onerror = () => {}));
        isClosing = false;
        hasBootstrapped = false;
        exports_115(
          "workerRuntimeGlobalProperties",
          (workerRuntimeGlobalProperties = {
            self: globals_ts_1.readOnly(globalThis),
            onmessage: globals_ts_1.writable(onmessage),
            onerror: globals_ts_1.writable(onerror),
            // TODO: should be readonly?
            close: globals_ts_1.nonEnumerable(close),
            postMessage: globals_ts_1.writable(postMessage),
            workerMessageRecvCallback: globals_ts_1.nonEnumerable(
              workerMessageRecvCallback
            ),
          })
        );
      },
    };
  }
);
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// TODO(ry) Combine this implementation with //deno_typescript/compiler_main.js
System.register(
  "cli/js/compiler.ts",
  [
    "$deno$/compiler/ts_global.d.ts",
    "$deno$/compiler/bootstrap.ts",
    "$deno$/compiler/bundler.ts",
    "$deno$/compiler/host.ts",
    "$deno$/compiler/imports.ts",
    "$deno$/compiler/util.ts",
    "$deno$/diagnostics_util.ts",
    "$deno$/util.ts",
    "$deno$/runtime_worker.ts",
  ],
  function (exports_116, context_116) {
    "use strict";
    let bootstrap_ts_2,
      bundler_ts_2,
      host_ts_2,
      imports_ts_1,
      util_ts_32,
      diagnostics_util_ts_1,
      util_ts_33,
      util,
      runtime_worker_ts_1;
    const __moduleName = context_116 && context_116.id;
    async function compile(request) {
      const {
        bundle,
        config,
        configPath,
        outFile,
        rootNames,
        target,
      } = request;
      util.log(">>> compile start", {
        rootNames,
        type: util_ts_32.CompilerRequestType[request.type],
      });
      // When a programme is emitted, TypeScript will call `writeFile` with
      // each file that needs to be emitted.  The Deno compiler host delegates
      // this, to make it easier to perform the right actions, which vary
      // based a lot on the request.  For a `Compile` request, we need to
      // cache all the files in the privileged side if we aren't bundling,
      // and if we are bundling we need to enrich the bundle and either write
      // out the bundle or log it to the console.
      const state = {
        type: request.type,
        bundle,
        host: undefined,
        outFile,
        rootNames,
      };
      const writeFile = util_ts_32.createWriteFile(state);
      const host = (state.host = new host_ts_2.Host({
        bundle,
        target,
        writeFile,
      }));
      let diagnostics;
      // if there is a configuration supplied, we need to parse that
      if (config && config.length && configPath) {
        const configResult = host.configure(configPath, config);
        diagnostics = util_ts_32.processConfigureResponse(
          configResult,
          configPath
        );
      }
      // This will recursively analyse all the code for other imports,
      // requesting those from the privileged side, populating the in memory
      // cache which will be used by the host, before resolving.
      const resolvedRootModules = await imports_ts_1.processImports(
        rootNames.map((rootName) => [rootName, rootName]),
        undefined,
        bundle || host.getCompilationSettings().checkJs
      );
      let emitSkipped = true;
      // if there was a configuration and no diagnostics with it, we will continue
      // to generate the program and possibly emit it.
      if (!diagnostics || (diagnostics && diagnostics.length === 0)) {
        const options = host.getCompilationSettings();
        const program = ts.createProgram({
          rootNames,
          options,
          host,
          oldProgram: bootstrap_ts_2.TS_SNAPSHOT_PROGRAM,
        });
        diagnostics = ts
          .getPreEmitDiagnostics(program)
          .filter(({ code }) => !util_ts_32.ignoredDiagnostics.includes(code));
        // We will only proceed with the emit if there are no diagnostics.
        if (diagnostics && diagnostics.length === 0) {
          if (bundle) {
            // we only support a single root module when bundling
            util_ts_33.assert(resolvedRootModules.length === 1);
            // warning so it goes to stderr instead of stdout
            console.warn(`Bundling "${resolvedRootModules[0]}"`);
            bundler_ts_2.setRootExports(program, resolvedRootModules[0]);
          }
          const emitResult = program.emit();
          emitSkipped = emitResult.emitSkipped;
          // emitResult.diagnostics is `readonly` in TS3.5+ and can't be assigned
          // without casting.
          diagnostics = emitResult.diagnostics;
        }
      }
      const result = {
        emitSkipped,
        diagnostics: diagnostics.length
          ? diagnostics_util_ts_1.fromTypeScriptDiagnostic(diagnostics)
          : undefined,
      };
      util.log("<<< compile end", {
        rootNames,
        type: util_ts_32.CompilerRequestType[request.type],
      });
      return result;
    }
    async function runtimeCompile(request) {
      const { rootName, sources, options, bundle, target } = request;
      util.log(">>> runtime compile start", {
        rootName,
        bundle,
        sources: sources ? Object.keys(sources) : undefined,
      });
      // resolve the root name, if there are sources, the root name does not
      // get resolved
      const resolvedRootName = sources
        ? rootName
        : imports_ts_1.resolveModules([rootName])[0];
      // if there are options, convert them into TypeScript compiler options,
      // and resolve any external file references
      let convertedOptions;
      let additionalFiles;
      if (options) {
        const result = util_ts_32.convertCompilerOptions(options);
        convertedOptions = result.options;
        additionalFiles = result.files;
      }
      const checkJsImports =
        bundle || (convertedOptions && convertedOptions.checkJs);
      // recursively process imports, loading each file into memory.  If there
      // are sources, these files are pulled out of the there, otherwise the
      // files are retrieved from the privileged side
      const rootNames = sources
        ? imports_ts_1.processLocalImports(
            sources,
            [[resolvedRootName, resolvedRootName]],
            undefined,
            checkJsImports
          )
        : await imports_ts_1.processImports(
            [[resolvedRootName, resolvedRootName]],
            undefined,
            checkJsImports
          );
      if (additionalFiles) {
        // any files supplied in the configuration are resolved externally,
        // even if sources are provided
        const resolvedNames = imports_ts_1.resolveModules(additionalFiles);
        rootNames.push(
          ...(await imports_ts_1.processImports(
            resolvedNames.map((rn) => [rn, rn]),
            undefined,
            checkJsImports
          ))
        );
      }
      const state = {
        type: request.type,
        bundle,
        host: undefined,
        rootNames,
        sources,
        emitMap: {},
        emitBundle: undefined,
      };
      const writeFile = util_ts_32.createWriteFile(state);
      const host = (state.host = new host_ts_2.Host({
        bundle,
        target,
        writeFile,
      }));
      const compilerOptions = [host_ts_2.defaultRuntimeCompileOptions];
      if (convertedOptions) {
        compilerOptions.push(convertedOptions);
      }
      if (bundle) {
        compilerOptions.push(host_ts_2.defaultBundlerOptions);
      }
      host.mergeOptions(...compilerOptions);
      const program = ts.createProgram({
        rootNames,
        options: host.getCompilationSettings(),
        host,
        oldProgram: bootstrap_ts_2.TS_SNAPSHOT_PROGRAM,
      });
      if (bundle) {
        bundler_ts_2.setRootExports(program, rootNames[0]);
      }
      const diagnostics = ts
        .getPreEmitDiagnostics(program)
        .filter(({ code }) => !util_ts_32.ignoredDiagnostics.includes(code));
      const emitResult = program.emit();
      util_ts_33.assert(
        emitResult.emitSkipped === false,
        "Unexpected skip of the emit."
      );
      util_ts_33.assert(state.emitMap);
      util.log("<<< runtime compile finish", {
        rootName,
        sources: sources ? Object.keys(sources) : undefined,
        bundle,
        emitMap: Object.keys(state.emitMap),
      });
      const maybeDiagnostics = diagnostics.length
        ? diagnostics_util_ts_1.fromTypeScriptDiagnostic(diagnostics).items
        : undefined;
      if (bundle) {
        return [maybeDiagnostics, state.emitBundle];
      } else {
        return [maybeDiagnostics, state.emitMap];
      }
    }
    function runtimeTranspile(request) {
      const result = {};
      const { sources, options } = request;
      const compilerOptions = options
        ? Object.assign(
            {},
            host_ts_2.defaultTranspileOptions,
            util_ts_32.convertCompilerOptions(options).options
          )
        : host_ts_2.defaultTranspileOptions;
      for (const [fileName, inputText] of Object.entries(sources)) {
        const { outputText: source, sourceMapText: map } = ts.transpileModule(
          inputText,
          {
            fileName,
            compilerOptions,
          }
        );
        result[fileName] = { source, map };
      }
      return Promise.resolve(result);
    }
    async function tsCompilerOnMessage({ data: request }) {
      switch (request.type) {
        case util_ts_32.CompilerRequestType.Compile: {
          const result = await compile(request);
          globalThis.postMessage(result);
          break;
        }
        case util_ts_32.CompilerRequestType.RuntimeCompile: {
          const result = await runtimeCompile(request);
          globalThis.postMessage(result);
          break;
        }
        case util_ts_32.CompilerRequestType.RuntimeTranspile: {
          const result = await runtimeTranspile(request);
          globalThis.postMessage(result);
          break;
        }
        default:
          util.log(
            `!!! unhandled CompilerRequestType: ${request.type} (${
              util_ts_32.CompilerRequestType[request.type]
            })`
          );
      }
      // Currently Rust shuts down worker after single request
    }
    async function wasmCompilerOnMessage({ data: binary }) {
      const buffer = util_ts_32.base64ToUint8Array(binary);
      // @ts-ignore
      const compiled = await WebAssembly.compile(buffer);
      util.log(">>> WASM compile start");
      const importList = Array.from(
        // @ts-ignore
        new Set(
          WebAssembly.Module.imports(compiled).map(({ module }) => module)
        )
      );
      const exportList = Array.from(
        // @ts-ignore
        new Set(WebAssembly.Module.exports(compiled).map(({ name }) => name))
      );
      globalThis.postMessage({ importList, exportList });
      util.log("<<< WASM compile end");
      // Currently Rust shuts down worker after single request
    }
    function bootstrapTsCompilerRuntime() {
      runtime_worker_ts_1.bootstrapWorkerRuntime("TS");
      globalThis.onmessage = tsCompilerOnMessage;
    }
    function bootstrapWasmCompilerRuntime() {
      runtime_worker_ts_1.bootstrapWorkerRuntime("WASM");
      globalThis.onmessage = wasmCompilerOnMessage;
    }
    return {
      setters: [
        function (_2) {},
        function (bootstrap_ts_2_1) {
          bootstrap_ts_2 = bootstrap_ts_2_1;
        },
        function (bundler_ts_2_1) {
          bundler_ts_2 = bundler_ts_2_1;
        },
        function (host_ts_2_1) {
          host_ts_2 = host_ts_2_1;
        },
        function (imports_ts_1_1) {
          imports_ts_1 = imports_ts_1_1;
        },
        function (util_ts_32_1) {
          util_ts_32 = util_ts_32_1;
        },
        function (diagnostics_util_ts_1_1) {
          diagnostics_util_ts_1 = diagnostics_util_ts_1_1;
        },
        function (util_ts_33_1) {
          util_ts_33 = util_ts_33_1;
          util = util_ts_33_1;
        },
        function (runtime_worker_ts_1_1) {
          runtime_worker_ts_1 = runtime_worker_ts_1_1;
        },
      ],
      execute: function () {
        // Removes the `__proto__` for security reasons.  This intentionally makes
        // Deno non compliant with ECMA-262 Annex B.2.2.1
        //
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        delete Object.prototype.__proto__;
        Object.defineProperties(globalThis, {
          bootstrapWasmCompilerRuntime: {
            value: bootstrapWasmCompilerRuntime,
            enumerable: false,
            writable: false,
            configurable: false,
          },
          bootstrapTsCompilerRuntime: {
            value: bootstrapTsCompilerRuntime,
            enumerable: false,
            writable: false,
            configurable: false,
          },
        });
      },
    };
  }
);
