// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
(() => {
  const GLOBAL_NAMESPACE = "Deno";
  const CORE_NAMESPACE = "core";
  // Available on start due to bindings.
  const Deno = globalThis[GLOBAL_NAMESPACE];
  const core = Deno[CORE_NAMESPACE];

  let logDebug = false;
  let logSource = "JS";

  function setLogDebug(debug, source) {
    logDebug = debug;
    if (source) {
      logSource = source;
    }
  }

  function log(...args) {
    if (logDebug) {
      // if we destructure `console` off `globalThis` too early, we don't bind to
      // the right console, therefore we don't log anything out.
      globalThis.console.log(`DEBUG ${logSource} -`, ...args);
    }
  }

  function assert(cond, msg = "assert") {
    if (!cond) {
      throw Error(msg);
    }
  }

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

  function notImplemented() {
    throw new Error("not implemented");
  }

  function immutableDefine(obj, prop, value) {
    Object.defineProperty(obj, prop, {
      value,
      configurable: false,
      writable: false,
    });
  }

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

  // `.apply` can actually take a typed array, though the type system doesn't
  // really support it, so we have to "hack" it a bit to get past some of the
  // strict type checks.
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
         0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
         0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
         0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
         0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,  0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
         1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,  9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,9,
         7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,  7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,
         8,8,2,2,2,2,2,2,2,2,2,2,2,2,2,2,  2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,
        10,3,3,3,3,3,3,3,3,3,3,3,3,4,3,3, 11,6,6,6,5,8,8,8,8,8,8,8,8,8,8,8
      ][input[i]];
      codepoint =
        state !== 0
          ? (input[i] & 0x3f) | (codepoint << 6)
          : (0xff >> type) & input[i];
      // prettier-ignore
      state = [
         0,12,24,36,60,96,84,12,12,12,48,72, 12,12,12,12,12,12,12,12,12,12,12,12,
        12, 0,12,12,12,12,12, 0,12, 0,12,12, 12,24,12,12,12,12,12,24,12,24,12,12,
        12,12,12,12,12,12,12,24,12,12,12,12, 12,24,12,12,12,12,12,12,12,24,12,12,
        12,12,12,12,12,12,12,36,12,36,12,12, 12,36,12,12,12,12,12,36,12,36,12,12,
        12,36,12,12,12,12,12,12,12,12,12,12
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

  const base64 = (function () {
    // Forked from https://github.com/beatgammit/base64-js
    // Copyright (c) 2014 Jameson Little. MIT License.

    const lookup = [];
    const revLookup = [];

    const code =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (let i = 0, len = code.length; i < len; ++i) {
      lookup[i] = code[i];
      revLookup[code.charCodeAt(i)] = i;
    }

    // Support decoding URL-safe base64 strings, as Node.js does.
    // See: https://en.wikipedia.org/wiki/Base64#URL_applications
    revLookup["-".charCodeAt(0)] = 62;
    revLookup["_".charCodeAt(0)] = 63;

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
    return {
      fromByteArray,
      toByteArray,
    };
  })();

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

  const CONTINUE = null;
  const END_OF_STREAM = -1;
  const FINISHED = -1;

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

  class UTF8Encoder {
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
      throw new TypeError("The string to be decoded is not correctly encoded");
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

  class SingleByteDecoder {
    #fatal = false;
    #index = 0;

    constructor(index, { ignoreBOM = false, fatal = false } = {}) {
      if (ignoreBOM) {
        throw new TypeError("Ignoring the BOM is available only with utf-8.");
      }
      this.#fatal = fatal;
      this.#index = index;
    }
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
  }

  // The encodingMap is a hash of labels that are indexed by the conical
  // encoding.
  const encodingMap = {
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
  const encodings = new Map();
  for (const key of Object.keys(encodingMap)) {
    const labels = encodingMap[key];
    for (const label of labels) {
      encodings.set(label, key);
    }
  }

  // A map of functions that return new instances of a decoder indexed by the
  // encoding type.
  const decoders = new Map();

  // Single byte decoders are an array of code point lookups
  const encodingIndexes = new Map();
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

  function codePointsToString(codePoints) {
    let s = "";
    for (const cp of codePoints) {
      s += String.fromCodePoint(cp);
    }
    return s;
  }

  class Stream {
    #tokens = [];

    constructor(tokens) {
      this.#tokens = [...tokens];
      this.#tokens.reverse();
    }

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
  }

  function isEitherArrayBuffer(x) {
    return x instanceof SharedArrayBuffer || x instanceof ArrayBuffer;
  }

  class TextDecoder {
    #encoding = "";

    get encoding() {
      return this.#encoding;
    }
    fatal = false;
    ignoreBOM = false;

    constructor(label = "utf-8", options = { fatal: false }) {
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
        throw new TypeError(`Internal decoder ('${encoding}') not found.`);
      }
      this.#encoding = encoding;
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
        return core.decode(bytes);
      }

      // For performance reasons we utilise a highly optimised decoder instead of
      // the general decoder.
      if (this.#encoding === "utf-8") {
        return decodeUtf8(bytes, this.fatal, this.ignoreBOM);
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
  }

  class TextEncoder {
    encoding = "utf-8";
    encode(input = "") {
      // Deno.core.encode() provides very efficient utf-8 encoding
      if (this.encoding === "utf-8") {
        return core.encode(input);
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
  }
  // END OF TEXT ENCODER CODE

  const ErrorKind = {
    NotFound: 1,
    PermissionDenied: 2,
    ConnectionRefused: 3,
    ConnectionReset: 4,
    ConnectionAborted: 5,
    NotConnected: 6,
    AddrInUse: 7,
    AddrNotAvailable: 8,
    BrokenPipe: 9,
    AlreadyExists: 10,
    InvalidData: 13,
    TimedOut: 14,
    Interrupted: 15,
    WriteZero: 16,
    UnexpectedEof: 17,
    BadResource: 18,
    Http: 19,
    URIError: 20,
    TypeError: 21,
    Other: 22,
  };

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

  class NotFound extends Error {
    constructor(msg) {
      super(msg);
      this.name = "NotFound";
    }
  }
  class PermissionDenied extends Error {
    constructor(msg) {
      super(msg);
      this.name = "PermissionDenied";
    }
  }
  class ConnectionRefused extends Error {
    constructor(msg) {
      super(msg);
      this.name = "ConnectionRefused";
    }
  }
  class ConnectionReset extends Error {
    constructor(msg) {
      super(msg);
      this.name = "ConnectionReset";
    }
  }
  class ConnectionAborted extends Error {
    constructor(msg) {
      super(msg);
      this.name = "ConnectionAborted";
    }
  }
  class NotConnected extends Error {
    constructor(msg) {
      super(msg);
      this.name = "NotConnected";
    }
  }
  class AddrInUse extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AddrInUse";
    }
  }
  class AddrNotAvailable extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AddrNotAvailable";
    }
  }
  class BrokenPipe extends Error {
    constructor(msg) {
      super(msg);
      this.name = "BrokenPipe";
    }
  }
  class AlreadyExists extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AlreadyExists";
    }
  }
  class InvalidData extends Error {
    constructor(msg) {
      super(msg);
      this.name = "InvalidData";
    }
  }
  class TimedOut extends Error {
    constructor(msg) {
      super(msg);
      this.name = "TimedOut";
    }
  }
  class Interrupted extends Error {
    constructor(msg) {
      super(msg);
      this.name = "Interrupted";
    }
  }
  class WriteZero extends Error {
    constructor(msg) {
      super(msg);
      this.name = "WriteZero";
    }
  }
  class UnexpectedEof extends Error {
    constructor(msg) {
      super(msg);
      this.name = "UnexpectedEof";
    }
  }
  class BadResource extends Error {
    constructor(msg) {
      super(msg);
      this.name = "BadResource";
    }
  }
  class Http extends Error {
    constructor(msg) {
      super(msg);
      this.name = "Http";
    }
  }

  const errors = {
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
  };

  // Using an object without a prototype because `Map` was causing GC problems.
  const promiseTable = Object.create(null);

  // Note it's important that promiseId starts at 1 instead of 0, because sync
  // messages are indicated with promiseId 0. If we ever add wrap around logic for
  // overflows, this should be taken into account.
  let _nextPromiseId = 1;

  const decoder = new TextDecoder();

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
      throw new errors.InvalidData("BadMessage");
    }

    return {
      promiseId,
      arg,
      result,
      err,
    };
  }

  function unwrapResponseMinimal(res) {
    if (res.err != null) {
      throw new (getErrorClass(res.err.kind))(res.err.message);
    }
    return res.result;
  }

  const scratch32 = new Int32Array(3);
  const scratchBytes = new Uint8Array(
    scratch32.buffer,
    scratch32.byteOffset,
    scratch32.byteLength
  );
  assert(scratchBytes.byteLength === scratch32.length * 4);

  function asyncMsgFromRustMinimal(ui8) {
    const record = recordFromBufMinimal(ui8);
    const { promiseId } = record;
    const promise = promiseTable[promiseId];
    delete promiseTable[promiseId];
    assert(promise);
    promise.resolve(record);
  }

  async function sendAsyncMinimal(opId, arg, zeroCopy) {
    const promiseId = nextPromiseId(); // AKA cmdId
    scratch32[0] = promiseId;
    scratch32[1] = arg;
    scratch32[2] = 0; // result
    const promise = createResolvable();
    const buf = core.dispatch(opId, scratchBytes, zeroCopy);
    if (buf) {
      const record = recordFromBufMinimal(buf);
      // Sync result.
      promise.resolve(record);
    } else {
      // Async result.
      promiseTable[promiseId] = promise;
    }

    const res = await promise;
    return unwrapResponseMinimal(res);
  }

  function sendSyncMinimal(opId, arg, zeroCopy) {
    scratch32[0] = 0; // promiseId 0 indicates sync
    scratch32[1] = arg;
    const res = core.dispatch(opId, scratchBytes, zeroCopy);
    const resRecord = recordFromBufMinimal(res);
    return unwrapResponseMinimal(resRecord);
  }

  function jsonDecode(ui8) {
    const s = core.decode(ui8);
    return JSON.parse(s);
  }

  function jsonEncode(args) {
    const s = JSON.stringify(args);
    return core.encode(s);
  }

  function unwrapResponseJson(res) {
    if (res.err != null) {
      throw new (getErrorClass(res.err.kind))(res.err.message);
    }
    assert(res.ok != null);
    return res.ok;
  }

  function asyncMsgFromRustJson(resUi8) {
    const res = jsonDecode(resUi8);
    assert(res.promiseId != null);

    const promise = promiseTable[res.promiseId];
    assert(promise != null);
    delete promiseTable[res.promiseId];
    promise.resolve(res);
  }

  function sendSyncJson(opName, args = {}, zeroCopy) {
    const opId = OPS_CACHE[opName];
    log("sendSync", opName, opId);
    const argsUi8 = jsonEncode(args);
    const resUi8 = core.dispatch(opId, argsUi8, zeroCopy);
    assert(resUi8 != null);
    const res = jsonDecode(resUi8);
    assert(res.promiseId == null);
    return unwrapResponseJson(res);
  }

  async function sendAsyncJson(opName, args = {}, zeroCopy) {
    const opId = OPS_CACHE[opName];
    log("sendAsync", opName, opId);
    const promiseId = nextPromiseId();
    args = Object.assign(args, { promiseId });
    const promise = createResolvable();

    const argsUi8 = jsonEncode(args);
    const buf = core.dispatch(opId, argsUi8, zeroCopy);
    if (buf) {
      // Sync result.
      const res = jsonDecode(buf);
      promise.resolve(res);
    } else {
      // Async result.
      promiseTable[promiseId] = promise;
    }

    const res = await promise;
    return unwrapResponseJson(res);
  }

  const build = {
    arch: "",
    os: "",
  };

  function setBuildInfo(os, arch) {
    build.os = os;
    build.arch = arch;

    Object.freeze(build);
  }

  const version = {
    deno: "",
    v8: "",
    typescript: "",
  };

  function setVersions(denoVersion, v8Version, tsVersion) {
    version.deno = denoVersion;
    version.v8 = v8Version;
    version.typescript = tsVersion;

    Object.freeze(version);
  }

  const internalSymbol = Symbol("Deno.internal");

  // The object where all the internal fields for testing will be living.
  const internalObject = {};

  // Register a field to internalObject for test access,
  // through Deno[Deno.symbols.internal][name].
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function exposeForTest(name, value) {
    Object.defineProperty(internalObject, name, {
      value,
      enumerable: false,
    });
  }

  function opStart() {
    return sendSyncJson("op_start");
  }
  function metrics() {
    return sendSyncJson("op_metrics");
  }

  let OPS_CACHE;

  function getAsyncHandler(opName) {
    switch (opName) {
      case "op_write":
      case "op_read":
        return asyncMsgFromRustMinimal;
      default:
        return asyncMsgFromRustJson;
    }
  }

  const bytesSymbol = Symbol("bytes");

  function containsOnlyASCII(str) {
    if (typeof str !== "string") {
      return false;
    }
    return /^[\x00-\x7F]*$/.test(str);
  }

  function convertLineEndingsToNative(s) {
    const nativeLineEnd = build.os == "win" ? "\r\n" : "\n";

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
    const enc = new TextEncoder();
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
    //   return new ReadableStream<Uint8Array>({
    //     start: (
    //       controller: domTypes.ReadableStreamDefaultController<Uint8Array>
    //     ): void => {
    //       controller.enqueue(blobBytes);
    //       controller.close();
    //     },
    //   }) as domTypes.ReadableStream<Uint8Array>;
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

  // A WeakMap holding blob to byte array mapping.
  // Ensures it does not impact garbage collection.
  const blobBytesWeakMap = new WeakMap();

  class DenoBlob {
    //   [bytesSymbol]: Uint8Array;
    size = 0;
    type = "";

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
      return new DenoBlob([this[bytesSymbol].slice(start, end)], {
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

  class DomFileImpl extends DenoBlob {
    constructor(fileBits, fileName, options) {
      const { lastModified = Date.now(), ...blobPropertyBag } = options ?? {};
      super(fileBits, blobPropertyBag);

      // 4.1.2.1 Replace any "/" character (U+002F SOLIDUS)
      // with a ":" (U + 003A COLON)
      this.name = String(fileName).replace(/\u002F/g, "\u003A");
      // 4.1.3.3 If lastModified is not provided, set lastModified to the current
      // date and time represented in number of milliseconds since the Unix Epoch.
      this.lastModified = lastModified;
    }
  }

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

  function requiredArguments(name, length, required) {
    if (length < required) {
      const errMsg = `${name} requires at least ${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present`;
      throw new TypeError(errMsg);
    }
  }

  function hasOwnProperty(obj, v) {
    if (obj == null) {
      return false;
    }
    return Object.prototype.hasOwnProperty.call(obj, v);
  }

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

  /** A helper function which ensures accessors are enumerable, as they normally
   * are not. */
  function defineEnumerableProps(Ctor, props) {
    for (const prop of props) {
      Reflect.defineProperty(Ctor.prototype, prop, { enumerable: true });
    }
  }

  const eventData = new WeakMap();

  // accessors for non runtime visible data

  function getDispatched(event) {
    return Boolean(eventData.get(event)?.dispatched);
  }

  function getPath(event) {
    return eventData.get(event)?.path ?? [];
  }

  function getStopImmediatePropagation(event) {
    return Boolean(eventData.get(event)?.stopImmediatePropagation);
  }

  function setCurrentTarget(event, value) {
    event.currentTarget = value;
  }

  function setDispatched(event, value) {
    const data = eventData.get(event);
    if (data) {
      data.dispatched = value;
    }
  }

  function setEventPhase(event, value) {
    event.eventPhase = value;
  }

  function setInPassiveListener(event, value) {
    const data = eventData.get(event);
    if (data) {
      data.inPassiveListener = value;
    }
  }

  function setPath(event, value) {
    const data = eventData.get(event);
    if (data) {
      data.path = value;
    }
  }

  function setRelatedTarget(event, value) {
    if ("relatedTarget" in event) {
      event.relatedTarget = value;
    }
  }

  function setTarget(event, value) {
    event.target = value;
  }

  function setStopImmediatePropagation(event, value) {
    const data = eventData.get(event);
    if (data) {
      data.stopImmediatePropagation = value;
    }
  }

  // Type guards that widen the event type

  function hasRelatedTarget(event) {
    return "relatedTarget" in event;
  }

  function isTrusted(event) {
    return eventData.get(event).isTrusted;
  }

  class EventImpl {
    // The default value is `false`.
    // Use `defineProperty` to define on each instance, NOT on the prototype.
    //   isTrusted!: boolean;

    #canceledFlag = false;
    #stopPropagationFlag = false;
    #attributes = {};

    constructor(type, eventInitDict = {}) {
      requiredArguments("Event", arguments.length, 1);
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

      assert(this.currentTarget);
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

      for (let index = currentTargetIndex + 1; index < path.length; index++) {
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
  }

  defineEnumerableProps(EventImpl, [
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

  class CustomEventImpl extends EventImpl {
    #detail = "";

    constructor(type, eventInitDict = {}) {
      super(type, eventInitDict);
      requiredArguments("CustomEvent", arguments.length, 1);
      const { detail } = eventInitDict;
      this.#detail = detail;
    }

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    get detail() {
      return this.#detail;
    }

    get [Symbol.toStringTag]() {
      return "CustomEvent";
    }
  }

  Reflect.defineProperty(CustomEventImpl.prototype, "detail", {
    enumerable: true,
  });

  // This module follows most of the WHATWG Living Standard for the DOM logic.
  // Many parts of the DOM are not implemented in Deno, but the logic for those
  // parts still exists.  This means you will observe a lot of strange structures
  // and impossible logic branches based on what Deno currently supports.
  class DOMException extends Error {
    #name = "";

    constructor(message = "", name = "Error") {
      super(message);
      this.#name = name;
    }

    get name() {
      return this.#name;
    }
  }

  // This is currently the only node type we are using, so instead of implementing
  // the whole of the Node interface at the moment, this just gives us the one
  // value to power the standards based logic
  const DOCUMENT_FRAGMENT_NODE = 11;

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

    getPath(eventImpl).push({
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

    setDispatched(eventImpl, true);

    targetOverride = targetOverride ?? targetImpl;
    const eventRelatedTarget = hasRelatedTarget(eventImpl)
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
      const path = getPath(eventImpl);
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

      setEventPhase(eventImpl, Event.CAPTURING_PHASE);

      for (let i = path.length - 1; i >= 0; --i) {
        const tuple = path[i];

        if (tuple.target === null) {
          invokeEventListeners(tuple, eventImpl);
        }
      }

      for (let i = 0; i < path.length; i++) {
        const tuple = path[i];

        if (tuple.target !== null) {
          setEventPhase(eventImpl, Event.AT_TARGET);
        } else {
          setEventPhase(eventImpl, Event.BUBBLING_PHASE);
        }

        if (
          (eventImpl.eventPhase === Event.BUBBLING_PHASE &&
            eventImpl.bubbles) ||
          eventImpl.eventPhase === Event.AT_TARGET
        ) {
          invokeEventListeners(tuple, eventImpl);
        }
      }
    }

    setEventPhase(eventImpl, Event.NONE);
    setCurrentTarget(eventImpl, null);
    setPath(eventImpl, []);
    setDispatched(eventImpl, false);
    eventImpl.cancelBubble = false;
    setStopImmediatePropagation(eventImpl, false);

    if (clearTargets) {
      setTarget(eventImpl, null);
      setRelatedTarget(eventImpl, null);
    }

    // TODO: invoke activation targets if HTML nodes will be implemented
    // if (activationTarget !== null) {
    //   if (!eventImpl.defaultPrevented) {
    //     activationTarget._activationBehavior();
    //   }
    // }

    return !eventImpl.defaultPrevented;
  }

  const streams = (function () {
    const shared = (function () {
      const state_ = Symbol("state_");
      const storedError_ = Symbol("storedError_");

      function isInteger(value) {
        if (!isFinite(value)) {
          // covers NaN, +Infinity and -Infinity
          return false;
        }
        const absValue = Math.abs(value);
        return Math.floor(absValue) === absValue;
      }

      function isFiniteNonNegativeNumber(value) {
        if (!(typeof value === "number" && isFinite(value))) {
          // covers NaN, +Infinity and -Infinity
          return false;
        }
        return value >= 0;
      }

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

      function cloneArrayBuffer(
        srcBuffer,
        srcByteOffset,
        srcLength,
        cloneConstructor
      ) {
        // this function fudges the return type but SharedArrayBuffer is disabled for a while anyway
        return srcBuffer.slice(srcByteOffset, srcByteOffset + srcLength);
      }

      function transferArrayBuffer(buffer) {
        // This would in a JS engine context detach the buffer's backing store and return
        // a new ArrayBuffer with the same backing store, invalidating `buffer`,
        // i.e. a move operation in C++ parlance.
        // Sadly ArrayBuffer.transfer is yet to be implemented by a single browser vendor.
        return buffer.slice(0); // copies instead of moves
      }

      function copyDataBlockBytes(
        toBlock,
        toIndex,
        fromBlock,
        fromIndex,
        count
      ) {
        new Uint8Array(toBlock, toIndex, count).set(
          new Uint8Array(fromBlock, fromIndex, count)
        );
      }

      // helper memoisation map for object values
      // weak so it doesn't keep memoized versions of old objects indefinitely.
      const objectCloneMemo = new WeakMap();

      let sharedArrayBufferSupported_;
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
            if (
              supportsSharedArrayBuffer() &&
              value instanceof SharedArrayBuffer
            ) {
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
              return new value.constructor(
                clonedBuffer,
                value.byteOffset,
                length
              );
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

      function promiseCall(f, v, args) {
        // tslint:disable-line:ban-types
        try {
          const result = Function.prototype.apply.call(f, v, args);
          return Promise.resolve(result);
        } catch (err) {
          return Promise.reject(err);
        }
      }

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

      // ----

      const ControlledPromiseState = {
        Pending: "Pending",
        Resolved: "Resolved",
        Rejected: "Rejected",
      };

      function createControlledPromise() {
        const conProm = {
          state: ControlledPromiseState.Pending,
        };
        conProm.promise = new Promise(function (resolve, reject) {
          conProm.resolve = function (v) {
            conProm.state = ControlledPromiseState.Resolved;
            resolve(v);
          };
          conProm.reject = function (e) {
            conProm.state = ControlledPromiseState.Rejected;
            reject(e);
          };
        });
        return conProm;
      }

      return {
        state_,
        storedError_,
        isInteger,
        isFiniteNonNegativeNumber,
        isAbortSignal,
        invokeOrNoop,
        cloneArrayBuffer,
        transferArrayBuffer,
        copyDataBlockBytes,
        cloneValue,
        promiseCall,
        createAlgorithmFromUnderlyingMethod,
        validateAndNormalizeHighWaterMark,
        makeSizeAlgorithmFromSizeFunction,
        ControlledPromiseState,
        createControlledPromise,
      };
    })();

    class ReadableStream {
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
          setUpReadableStreamDefaultControllerFromUnderlyingSource(
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
          setUpReadableByteStreamControllerFromUnderlyingSource(
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
        // return rs.isReadableStreamLocked(this);
      }

      getReader(options) {
        // if (!rs.isReadableStream(this)) {
        //   throw new TypeError();
        // }
        // if (options === undefined) {
        //   options = {};
        // }
        // const { mode } = options;
        // if (mode === undefined) {
        //   return new ReadableStreamDefaultReader(this);
        // } else if (String(mode) === "byob") {
        //   return new SDReadableStreamBYOBReader(
        //     (this as unknown)
        //   );
        // }
        // throw RangeError("mode option must be undefined or `byob`");
      }

      cancel(reason) {
        // if (!rs.isReadableStream(this)) {
        //   return Promise.reject(new TypeError());
        // }
        // if (rs.isReadableStreamLocked(this)) {
        //   return Promise.reject(new TypeError("Cannot cancel a locked stream"));
        // }
        // return rs.readableStreamCancel(this, reason);
      }

      tee() {
        return readableStreamTee(this, false);
      }

      /* TODO reenable these methods when we bring in writableStreams and transport types
      pipeThrough<ResultType>(
        transform: rs.GenericTransformStream<OutputType, ResultType>,
        options: PipeOptions = {}
      ): rs.SDReadableStream<ResultType> {
        const { readable, writable } = transform;
        if (!rs.isReadableStream(this)) {
          throw new TypeError();
        }
        if (!ws.isWritableStream(writable)) {
          throw new TypeError("writable must be a WritableStream");
        }
        if (!rs.isReadableStream(readable)) {
          throw new TypeError("readable must be a ReadableStream");
        }
        if (options.signal !== undefined && !shared.isAbortSignal(options.signal)) {
          throw new TypeError("options.signal must be an AbortSignal instance");
        }
        if (rs.isReadableStreamLocked(this)) {
          throw new TypeError("Cannot pipeThrough on a locked stream");
        }
        if (ws.isWritableStreamLocked(writable)) {
          throw new TypeError("Cannot pipeThrough to a locked stream");
        }
  
        const pipeResult = pipeTo(this, writable, options);
        pipeResult.catch(() => {});
  
        return readable;
      }
  
      pipeTo(
        dest: ws.WritableStream<OutputType>,
        options: PipeOptions = {}
      ): Promise<void> {
        if (!rs.isReadableStream(this)) {
          return Promise.reject(new TypeError());
        }
        if (!ws.isWritableStream(dest)) {
          return Promise.reject(
            new TypeError("destination must be a WritableStream")
          );
        }
        if (options.signal !== undefined && !shared.isAbortSignal(options.signal)) {
          return Promise.reject(
            new TypeError("options.signal must be an AbortSignal instance")
          );
        }
        if (rs.isReadableStreamLocked(this)) {
          return Promise.reject(new TypeError("Cannot pipe from a locked stream"));
        }
        if (ws.isWritableStreamLocked(dest)) {
          return Promise.reject(new TypeError("Cannot pipe to a locked stream"));
        }
  
        return pipeTo(this, dest, options);
      }
      */
    }

    return {
      ReadableStream,
    };
  })();

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
        (eventImpl.eventPhase === Event.CAPTURING_PHASE && !capture) ||
        (eventImpl.eventPhase === Event.BUBBLING_PHASE && capture)
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
        setInPassiveListener(eventImpl, true);
      }

      if (typeof listener.callback === "object") {
        if (typeof listener.callback.handleEvent === "function") {
          listener.callback.handleEvent(eventImpl);
        }
      } else {
        listener.callback.call(eventImpl.currentTarget, eventImpl);
      }

      setInPassiveListener(eventImpl, false);

      if (getStopImmediatePropagation(eventImpl)) {
        return found;
      }
    }

    return found;
  }

  /** Invokes the listeners on a given event path with the supplied event.
   *
   * Ref: https://dom.spec.whatwg.org/#concept-event-listener-invoke */
  function invokeEventListeners(tuple, eventImpl) {
    const path = getPath(eventImpl);
    const tupleIndex = path.indexOf(tuple);
    for (let i = tupleIndex; i >= 0; i--) {
      const t = path[i];
      if (t.target) {
        setTarget(eventImpl, t.target);
        break;
      }
    }

    setRelatedTarget(eventImpl, tuple.relatedTarget);

    if (eventImpl.cancelBubble) {
      return;
    }

    setCurrentTarget(eventImpl, tuple.item);

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

  // Accessors for non-public data

  const eventTargetData = new WeakMap();

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

  class EventTargetImpl {
    constructor() {
      eventTargetData.set(this, getDefaultTargetData());
    }

    addEventListener(type, callback, options) {
      requiredArguments("EventTarget.addEventListener", arguments.length, 2);
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
      requiredArguments("EventTarget.removeEventListener", arguments.length, 2);

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
      requiredArguments("EventTarget.dispatchEvent", arguments.length, 1);
      const self = this ?? globalThis;

      const listeners = eventTargetData.get(self).listeners;
      if (!(event.type in listeners)) {
        return true;
      }

      if (getDispatched(event)) {
        throw new DOMException("Invalid event state.", "InvalidStateError");
      }

      if (event.eventPhase !== Event.NONE) {
        throw new DOMException("Invalid event state.", "InvalidStateError");
      }

      return dispatch(self, event);
    }

    get [Symbol.toStringTag]() {
      return "EventTarget";
    }

    getParent(_event) {
      return null;
    }
  }

  defineEnumerableProps(EventTargetImpl, [
    "addEventListener",
    "removeEventListener",
    "dispatchEvent",
  ]);

  const urls = new WeakMap();

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
      searchParams.append(decodeURIComponent(name), decodeURIComponent(value));
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

  class URLSearchParamsImpl {
    #params = [];

    constructor(init = "") {
      if (typeof init === "string") {
        handleStringInitialization(this, init);
        return;
      }

      if (Array.isArray(init) || isIterable(init)) {
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

    #updateSteps = () => {
      const url = urls.get(this);
      if (url == null) {
        return;
      }

      let query = this.toString();
      if (query === "") {
        query = null;
      }

      parts.get(url).query = query;
    };

    append(name, value) {
      requiredArguments("URLSearchParams.append", arguments.length, 2);
      this.#params.push([String(name), String(value)]);
      this.#updateSteps();
    }

    delete(name) {
      requiredArguments("URLSearchParams.delete", arguments.length, 1);
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
      requiredArguments("URLSearchParams.getAll", arguments.length, 1);
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
      requiredArguments("URLSearchParams.get", arguments.length, 1);
      name = String(name);
      for (const entry of this.#params) {
        if (entry[0] === name) {
          return entry[1];
        }
      }

      return null;
    }

    has(name) {
      requiredArguments("URLSearchParams.has", arguments.length, 1);
      name = String(name);
      return this.#params.some((entry) => entry[0] === name);
    }

    set(name, value) {
      requiredArguments("URLSearchParams.set", arguments.length, 2);

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
      this.#params.sort((a, b) => (a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1));
      this.#updateSteps();
    }

    forEach(callbackfn, thisArg) {
      requiredArguments("URLSearchParams.forEach", arguments.length, 1);

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
            `${encodeURIComponent(tuple[0])}=${encodeURIComponent(tuple[1])}`
        )
        .join("&");
    }
  }

  const patterns = {
    protocol: "(?:([a-z]+):)",
    authority: "(?://([^/?#]*))",
    path: "([^?#]*)",
    query: "(\\?[^#]*)",
    hash: "(#.*)",

    authentication: "(?:([^:]*)(?::([^@]*))?@)",
    hostname: "([^:]+)",
    port: "(?::(\\d+))",
  };

  const urlRegExp = new RegExp(
    `^${patterns.protocol}?${patterns.authority}?${patterns.path}${patterns.query}?${patterns.hash}?`
  );

  const authorityRegExp = new RegExp(
    `^${patterns.authentication}?${patterns.hostname}${patterns.port}?$`
  );

  const searchParamsMethods = ["append", "delete", "set"];

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
      (csprng.getRandomValues(new Uint8Array(1))[0] % 16).toString(16)
    );
  }

  // Keep it outside of URL to avoid any attempts of access.
  const blobURLMap = new Map();

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

  /** @internal */
  const parts = new WeakMap();

  class URLImpl {
    #searchParams = undefined;

    // [customInspect]() {
    //   const keys = [
    //     "href",
    //     "origin",
    //     "protocol",
    //     "username",
    //     "password",
    //     "host",
    //     "hostname",
    //     "port",
    //     "pathname",
    //     "hash",
    //     "search",
    //   ];
    //   const objectString = keys
    //     .map((key) => `${key}: "${this[key] || ""}"`)
    //     .join(", ");
    //   return `URL { ${objectString} }`;
    // }

    #updateSearchParams = () => {
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

      urls.set(searchParams, this);
    };

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

    constructor(url, base) {
      let baseParts;
      if (base) {
        baseParts = typeof base === "string" ? parse(base) : parts.get(base);
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

    toString() {
      return this.href;
    }

    toJSON() {
      return this.href;
    }

    // TODO(kevinkassimo): implement MediaSource version in the future.
    static createObjectURL(b) {
      const origin = globalThis.location.origin || "http://deno-opaque-origin";
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
  }

  // TODO(bartlomieju): temporary solution, must be fixed when moving
  // dispatches to separate crates
  function initOps() {
    OPS_CACHE = core.ops();
    for (const [name, opId] of Object.entries(OPS_CACHE)) {
      core.setAsyncHandler(opId, getAsyncHandler(name));
    }
    core.setMacrotaskCallback(handleTimerMacrotask);
  }

  function start(source) {
    initOps();
    // First we send an empty `Start` message to let the privileged side know we
    // are ready. The response should be a `StartRes` message containing the CLI
    // args and other info.
    const s = opStart();

    setVersions(s.denoVersion, s.v8Version, s.tsVersion);
    setBuildInfo(s.os, s.arch);
    setLogDebug(s.debugFlag, source);

    // setPrepareStackTrace(Error);
    return s;
  }

  function bindSignal(signo) {
    return sendSyncJson("op_signal_bind", { signo });
  }

  function pollSignal(rid) {
    return sendAsyncJson("op_signal_poll", { rid });
  }

  function unbindSignal(rid) {
    sendSyncJson("op_signal_unbind", { rid });
  }

  // From `kill -l`
  const LinuxSignal = {
    SIGHUP: 1,
    SIGINT: 2,
    SIGQUIT: 3,
    SIGILL: 4,
    SIGTRAP: 5,
    SIGABRT: 6,
    SIGBUS: 7,
    SIGFPE: 8,
    SIGKILL: 9,
    SIGUSR1: 10,
    SIGSEGV: 11,
    SIGUSR2: 12,
    SIGPIPE: 13,
    SIGALRM: 14,
    SIGTERM: 15,
    SIGSTKFLT: 16,
    SIGCHLD: 17,
    SIGCONT: 18,
    SIGSTOP: 19,
    SIGTSTP: 20,
    SIGTTIN: 21,
    SIGTTOU: 22,
    SIGURG: 23,
    SIGXCPU: 24,
    SIGXFSZ: 25,
    SIGVTALRM: 26,
    SIGPROF: 27,
    SIGWINCH: 28,
    SIGIO: 29,
    SIGPWR: 30,
    SIGSYS: 31,
  };

  // From `kill -l`
  const MacOSSignal = {
    SIGHUP: 1,
    SIGINT: 2,
    SIGQUIT: 3,
    SIGILL: 4,
    SIGTRAP: 5,
    SIGABRT: 6,
    SIGEMT: 7,
    SIGFPE: 8,
    SIGKILL: 9,
    SIGBUS: 10,
    SIGSEGV: 11,
    SIGSYS: 12,
    SIGPIPE: 13,
    SIGALRM: 14,
    SIGTERM: 15,
    SIGURG: 16,
    SIGSTOP: 17,
    SIGTSTP: 18,
    SIGCONT: 19,
    SIGCHLD: 20,
    SIGTTIN: 21,
    SIGTTOU: 22,
    SIGIO: 23,
    SIGXCPU: 24,
    SIGXFSZ: 25,
    SIGVTALRM: 26,
    SIGPROF: 27,
    SIGWINCH: 28,
    SIGINFO: 29,
    SIGUSR1: 30,
    SIGUSR2: 31,
  };

  const Signal = {};

  function setSignals() {
    if (build.os === "mac") {
      Object.assign(Signal, MacOSSignal);
    } else {
      Object.assign(Signal, LinuxSignal);
    }
  }

  function signal(signo) {
    if (build.os === "win") {
      throw new Error("not implemented!");
    }
    return new SignalStream(signo);
  }

  const signals = {
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
  };

  class SignalStream {
    #disposed = false;
    #pollingPromise = Promise.resolve(false);
    #rid = undefined;

    constructor(signo) {
      this.#rid = bindSignal(signo).rid;
      this.#loop();
    }

    #pollSignal = async () => {
      const res = await pollSignal(this.#rid);
      return res.done;
    };

    #loop = async () => {
      do {
        this.#pollingPromise = this.#pollSignal();
      } while (!(await this.#pollingPromise) && !this.#disposed);
    };

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
      unbindSignal(this.#rid);
    }
  }

  const csprng = (function () {
    function getRandomValues(typedArray) {
      assert(typedArray !== null, "Input must not be null");
      assert(typedArray.length <= 65536, "Input must not be longer than 65536");
      const ui8 = new Uint8Array(
        typedArray.buffer,
        typedArray.byteOffset,
        typedArray.byteLength
      );
      sendSyncJson("op_get_random_values", {}, ui8);
      return typedArray;
    }

    return {
      getRandomValues,
    };
  })();

  function loadavg() {
    return sendSyncJson("op_loadavg");
  }

  function hostname() {
    return sendSyncJson("op_hostname");
  }

  function osRelease() {
    return sendSyncJson("op_os_release");
  }

  function exit(code = 0) {
    sendSyncJson("op_exit", { code });
    throw new Error("Code not reachable");
  }

  function setEnv(key, value) {
    sendSyncJson("op_set_env", { key, value });
  }

  function getEnv(key) {
    return sendSyncJson("op_get_env", { key })[0];
  }

  function env(key) {
    if (key) {
      return getEnv(key);
    }
    const env = sendSyncJson("op_env");
    return new Proxy(env, {
      set(obj, prop, value) {
        setEnv(prop, value);
        return Reflect.set(obj, prop, value);
      },
    });
  }

  function dir(kind) {
    try {
      return sendSyncJson("op_get_dir", { kind });
    } catch (error) {
      if (error instanceof errors.PermissionDenied) {
        throw error;
      }
      return null;
    }
  }

  function execPath() {
    return sendSyncJson("op_exec_path");
  }

  function writable(value) {
    return {
      value,
      writable: true,
      enumerable: true,
      configurable: true,
    };
  }

  function nonEnumerable(value) {
    return {
      value,
      writable: true,
      configurable: true,
    };
  }

  function readOnly(value) {
    return {
      value,
      enumerable: true,
    };
  }

  function getterOnly(getter) {
    return {
      get: getter,
      enumerable: true,
    };
  }

  // Copyright Joyent, Inc. and other Node contributors. MIT license.
  // Forked from Node's lib/internal/cli_table.js

  const encoder = new TextEncoder();

  const tableChars = {
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

  const colorRegExp = /\u001b\[\d\d?m/g;

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
        const value = (rows[j][i] = hasOwnProperty(column, j) ? column[j] : "");
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
  const PromiseState = {
    Pending: 0,
    Fulfilled: 1,
    Rejected: 2,
  };

  const EOF = Symbol("EOF");

  // This is done because read/write are extremely performance sensitive.
  let OP_READ = -1;
  let OP_WRITE = -1;

  function readSync(rid, buffer) {
    if (buffer.length == 0) {
      return 0;
    }
    if (OP_READ < 0) {
      OP_READ = OPS_CACHE["op_read"];
    }
    const nread = sendSyncMinimal(OP_READ, rid, buffer);
    if (nread < 0) {
      throw new Error("read error");
    } else if (nread == 0) {
      return EOF;
    } else {
      return nread;
    }
  }

  async function read(rid, buffer) {
    if (buffer.length == 0) {
      return 0;
    }
    if (OP_READ < 0) {
      OP_READ = OPS_CACHE["op_read"];
    }
    const nread = await sendAsyncMinimal(OP_READ, rid, buffer);
    if (nread < 0) {
      throw new Error("read error");
    } else if (nread == 0) {
      return EOF;
    } else {
      return nread;
    }
  }

  function writeSync(rid, data) {
    if (OP_WRITE < 0) {
      OP_WRITE = OPS_CACHE["op_write"];
    }
    const result = sendSyncMinimal(OP_WRITE, rid, data);
    if (result < 0) {
      throw new Error("write error");
    } else {
      return result;
    }
  }

  async function write(rid, data) {
    if (OP_WRITE < 0) {
      OP_WRITE = OPS_CACHE["op_write"];
    }
    const result = await sendAsyncMinimal(OP_WRITE, rid, data);
    if (result < 0) {
      throw new Error("write error");
    } else {
      return result;
    }
  }

  function seekSync(rid, offset, whence) {
    return sendSyncJson("op_seek", { rid, offset, whence });
  }

  function seek(rid, offset, whence) {
    return sendAsyncJson("op_seek", { rid, offset, whence });
  }

  function opOpenSync(path, openMode, options) {
    const mode = options?.mode;
    return sendSyncJson("op_open", { path, options, openMode, mode });
  }

  function opOpen(path, openMode, options) {
    const mode = options?.mode;
    return sendAsyncJson("op_open", {
      path,
      options,
      openMode,
      mode,
    });
  }

  function openSync(path, modeOrOptions = "r") {
    let openMode = undefined;
    let options = undefined;

    if (typeof modeOrOptions === "string") {
      openMode = modeOrOptions;
    } else {
      checkOpenOptions(modeOrOptions);
      options = modeOrOptions;
    }

    const rid = opOpenSync(path, openMode, options);
    return new File(rid);
  }

  async function open(path, modeOrOptions = "r") {
    let openMode = undefined;
    let options = undefined;

    if (typeof modeOrOptions === "string") {
      openMode = modeOrOptions;
    } else {
      checkOpenOptions(modeOrOptions);
      options = modeOrOptions;
    }

    const rid = await opOpen(path, openMode, options);
    return new File(rid);
  }

  function createSync(path) {
    return openSync(path, "w+");
  }

  function create(path) {
    return open(path, "w+");
  }

  class File {
    constructor(rid) {
      this.rid = rid;
    }

    write(p) {
      return write(this.rid, p);
    }

    writeSync(p) {
      return writeSync(this.rid, p);
    }

    read(p) {
      return read(this.rid, p);
    }

    readSync(p) {
      return readSync(this.rid, p);
    }

    seek(offset, whence) {
      return seek(this.rid, offset, whence);
    }

    seekSync(offset, whence) {
      return seekSync(this.rid, offset, whence);
    }

    close() {
      close(this.rid);
    }
  }

  const stdin = new File(0);
  const stdout = new File(1);
  const stderr = new File(2);

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

  const DEFAULT_MAX_DEPTH = 4; // Default depth of logging nested objects
  const LINE_BREAKING_LENGTH = 80;
  const MAX_ITERABLE_LENGTH = 100;
  const MIN_GROUP_LENGTH = 6;
  const STR_ABBREVIATE_SIZE = 100;
  // Char codes
  const CHAR_PERCENT = 37; /* % */
  const CHAR_LOWERCASE_S = 115; /* s */
  const CHAR_LOWERCASE_D = 100; /* d */
  const CHAR_LOWERCASE_I = 105; /* i */
  const CHAR_LOWERCASE_F = 102; /* f */
  const CHAR_LOWERCASE_O = 111; /* o */
  const CHAR_UPPERCASE_O = 79; /* O */
  const CHAR_LOWERCASE_C = 99; /* c */

  const PROMISE_STRING_BASE_LENGTH = 12;

  class CSI {
    static kClear = "\x1b[1;1H";
    static kClearScreenDown = "\x1b[0J";
  }

  /* eslint-disable @typescript-eslint/no-use-before-define */

  function cursorTo(stream, _x, _y) {
    const uint8 = new TextEncoder().encode(CSI.kClear);
    stream.writeSync(uint8);
  }

  function clearScreenDown(stream) {
    const uint8 = new TextEncoder().encode(CSI.kClearScreenDown);
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

  function createTypedArrayString(typedArrayName, value, ctx, level, maxLevel) {
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

    if (state === PromiseState.Pending) {
      return "Promise { <pending> }";
    }

    const prefix = state === PromiseState.Fulfilled ? "" : "<rejected> ";

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
    if (customInspect in value && typeof value[customInspect] === "function") {
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
    } else if (isTypedArray(value)) {
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

  const countMap = new Map();
  const timerMap = new Map();
  const isConsoleInstance = Symbol("isConsoleInstance");

  class Console {
    #printFunc;
    indentLevel;
    [isConsoleInstance] = false;

    constructor(printFunc) {
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

    log = (...args) => {
      this.#printFunc(
        stringifyArgs(args, {
          indentLevel: this.indentLevel,
        }) + "\n",
        false
      );
    };

    debug = this.log;
    info = this.log;

    dir = (obj, options = {}) => {
      this.#printFunc(stringifyArgs([obj], options) + "\n", false);
    };

    dirxml = this.dir;

    warn = (...args) => {
      this.#printFunc(
        stringifyArgs(args, {
          indentLevel: this.indentLevel,
        }) + "\n",
        true
      );
    };

    error = this.warn;

    assert = (condition = false, ...args) => {
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

    count = (label = "default") => {
      label = String(label);

      if (countMap.has(label)) {
        const current = countMap.get(label) || 0;
        countMap.set(label, current + 1);
      } else {
        countMap.set(label, 1);
      }

      this.info(`${label}: ${countMap.get(label)}`);
    };

    countReset = (label = "default") => {
      label = String(label);

      if (countMap.has(label)) {
        countMap.set(label, 0);
      } else {
        this.warn(`Count for '${label}' does not exist`);
      }
    };

    table = (data, properties) => {
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
      const toTable = (header, body) => this.log(cliTable(header, body));
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

    time = (label = "default") => {
      label = String(label);

      if (timerMap.has(label)) {
        this.warn(`Timer '${label}' already exists`);
        return;
      }

      timerMap.set(label, Date.now());
    };

    timeLog = (label = "default", ...args) => {
      label = String(label);

      if (!timerMap.has(label)) {
        this.warn(`Timer '${label}' does not exists`);
        return;
      }

      const startTime = timerMap.get(label);
      const duration = Date.now() - startTime;

      this.info(`${label}: ${duration}ms`, ...args);
    };

    timeEnd = (label = "default") => {
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

    group = (...label) => {
      if (label.length > 0) {
        this.log(...label);
      }
      this.indentLevel += 2;
    };

    groupCollapsed = this.group;

    groupEnd = () => {
      if (this.indentLevel > 0) {
        this.indentLevel -= 2;
      }
    };

    clear = () => {
      this.indentLevel = 0;
      cursorTo(stdout, 0, 0);
      clearScreenDown(stdout);
    };

    trace = (...args) => {
      const message = stringifyArgs(args, { indentLevel: 0 });
      const err = {
        name: "Trace",
        message,
      };
      // @ts-ignore
      Error.captureStackTrace(err, this.trace);
      this.error(err.stack);
    };

    static [Symbol.hasInstance](instance) {
      return instance[isConsoleInstance];
    }
  }

  const customInspect = Symbol.for("Deno.customInspect");

  function inspect(value, { depth = DEFAULT_MAX_DEPTH } = {}) {
    if (typeof value === "string") {
      return value;
    } else {
      return stringify(value, new Set(), 0, depth);
    }
  }

  // Expose these fields to internalObject for tests.
  exposeForTest("Console", Console);
  exposeForTest("stringifyArgs", stringifyArgs);

  function resources() {
    const res = sendSyncJson("op_resources");
    const resources = {};
    for (const resourceTuple of res) {
      resources[resourceTuple[0]] = resourceTuple[1];
    }
    return resources;
  }

  function close(rid) {
    sendSyncJson("op_close", { rid });
  }

  function startRepl(historyFile) {
    return sendSyncJson("op_repl_start", { historyFile });
  }

  function readline(rid, prompt) {
    return sendAsyncJson("op_repl_readline", { rid, prompt });
  }

  function replLog(...args) {
    core.print(stringifyArgs(args) + "\n");
  }

  function replError(...args) {
    core.print(stringifyArgs(args) + "\n", true);
  }

  const helpMsg = [
    "_       Get last evaluation result",
    "_error  Get last thrown error",
    "exit    Exit the REPL",
    "help    Print this help message",
  ].join("\n");

  const replCommands = {
    exit: {
      get() {
        exit(0);
      },
    },
    help: {
      get() {
        return helpMsg;
      },
    },
  };

  // Error messages that allow users to continue input
  // instead of throwing an error to REPL
  // ref: https://github.com/v8/v8/blob/master/src/message-template.h
  // TODO(kevinkassimo): this list might not be comprehensive
  const recoverableErrorMessages = [
    "Unexpected end of input", // { or [ or (
    "Missing initializer in const declaration", // const a
    "Missing catch or finally after try", // try {}
    "missing ) after argument list", // console.log(1
    "Unterminated template literal", // `template
    // TODO(kevinkassimo): need a parser to handling errors such as:
    // "Missing } in template expression" // `${ or `${ a 123 }`
  ];

  function isRecoverableError(e) {
    return recoverableErrorMessages.includes(e.message);
  }

  let lastEvalResult = undefined;
  let lastThrownError = undefined;

  // Evaluate code.
  // Returns true if code is consumed (no error/irrecoverable error).
  // Returns false if error is recoverable
  function evaluate(code) {
    const [result, errInfo] = core.evalContext(code);
    if (!errInfo) {
      lastEvalResult = result;
      replLog(result);
    } else if (errInfo.isCompileError && isRecoverableError(errInfo.thrown)) {
      // Recoverable compiler error
      return false; // don't consume code.
    } else {
      lastThrownError = errInfo.thrown;
      if (errInfo.isNativeError) {
        const formattedError = core.formatError(errInfo.thrown);
        replError(formattedError);
      } else {
        replError("Thrown:", errInfo.thrown);
      }
    }
    return true;
  }

  async function replLoop() {
    const { console } = globalThis;
    Object.defineProperties(globalThis, replCommands);

    const historyFile = "deno_history.txt";
    const rid = startRepl(historyFile);

    const quitRepl = (exitCode) => {
      // Special handling in case user calls deno.close(3).
      try {
        close(rid); // close signals Drop on REPL and saves history.
      } catch {}
      exit(exitCode);
    };

    // Configure globalThis._ to give the last evaluation result.
    Object.defineProperty(globalThis, "_", {
      configurable: true,
      get: () => lastEvalResult,
      set: (value) => {
        Object.defineProperty(globalThis, "_", {
          value: value,
          writable: true,
          enumerable: true,
          configurable: true,
        });
        console.log("Last evaluation result is no longer saved to _.");
      },
    });

    // Configure globalThis._error to give the last thrown error.
    Object.defineProperty(globalThis, "_error", {
      configurable: true,
      get: () => lastThrownError,
      set: (value) => {
        Object.defineProperty(globalThis, "_error", {
          value: value,
          writable: true,
          enumerable: true,
          configurable: true,
        });
        console.log("Last thrown error is no longer saved to _error.");
      },
    });

    while (true) {
      let code = "";
      // Top level read
      try {
        code = await readline(rid, "> ");
        if (code.trim() === "") {
          continue;
        }
      } catch (err) {
        if (err.message === "EOF") {
          quitRepl(0);
        } else {
          // If interrupted, don't print error.
          if (err.message !== "Interrupted") {
            // e.g. this happens when we have deno.close(3).
            // We want to display the problem.
            const formattedError = core.formatError(err);
            replError(formattedError);
          }
          // Quit REPL anyways.
          quitRepl(1);
        }
      }
      // Start continued read
      while (!evaluate(code)) {
        code += "\n";
        try {
          code += await readline(rid, "  ");
        } catch (err) {
          // If interrupted on continued read,
          // abort this read instead of quitting.
          if (err.message === "Interrupted") {
            break;
          } else if (err.message === "EOF") {
            quitRepl(0);
          } else {
            // e.g. this happens when we have deno.close(3).
            // We want to display the problem.
            const formattedError = core.formatError(err);
            replError(formattedError);
            quitRepl(1);
          }
        }
      }
    }
  }

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

  class LocationImpl {
    #url = undefined;

    constructor(url) {
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

    toString() {
      return this.#url.toString();
    }

    ancestorOrigins = getDOMStringList([]);

    assign(_url) {
      throw notImplemented();
    }
    reload() {
      throw notImplemented();
    }
    replace(_url) {
      throw notImplemented();
    }
  }

  function setLocation(url) {
    globalThis.location = new LocationImpl(url);
    Object.freeze(globalThis.location);
  }

  class RBNode {
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
  }

  class RBTree {
    #comparator = undefined;
    #root = undefined;

    constructor(comparator) {
      this.#comparator = comparator;
      this.#root = null;
    }

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

            assert(gp);
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
                assert(gp);
                const dir2 = gp.right === p;

                if (isRed(sibling.getChild(last))) {
                  gp.setChild(dir2, doubleRotate(p, last));
                } else if (isRed(sibling.getChild(!last))) {
                  gp.setChild(dir2, singleRotate(p, last));
                }

                // ensure correct coloring
                const gpc = gp.getChild(dir2);
                assert(gpc);
                gpc.red = true;
                node.red = true;
                assert(gpc.left);
                gpc.left.red = false;
                assert(gpc.right);
                gpc.right.red = false;
              }
            }
          }
        }
      }

      // replace and remove if found
      if (found !== null) {
        found.data = node.data;
        assert(p);
        p.setChild(p.right === node, node.getChild(node.left === null));
      }

      // update root and make it black
      this.#root = head.right;
      if (this.#root !== null) {
        this.#root.red = false;
      }

      return found !== null;
    }
  }

  function isRed(node) {
    return node !== null && node.red;
  }

  function singleRotate(root, dir) {
    const save = root.getChild(!dir);
    assert(save);

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

  function stopGlobalTimer() {
    sendSyncJson("op_global_timer_stop");
  }

  async function startGlobalTimer(timeout) {
    await sendAsyncJson("op_global_timer", { timeout });
  }

  function now() {
    return sendSyncJson("op_now");
  }

  // Timeout values > TIMEOUT_MAX are set to 1.
  const TIMEOUT_MAX = 2 ** 31 - 1;

  let globalTimeoutDue = null;

  let nextTimerId = 1;
  const idMap = new Map();
  const dueTree = new RBTree((a, b) => a.due - b.due);

  function clearGlobalTimeout() {
    globalTimeoutDue = null;
    stopGlobalTimer();
  }

  let pendingEvents = 0;
  const pendingFireTimers = [];

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

  async function setGlobalTimeout(due, now) {
    // Since JS and Rust don't use the same clock, pass the time to rust as a
    // relative time value. On the Rust side we'll turn that into an absolute
    // value again.
    const timeout = due - now;
    assert(timeout >= 0);
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
    await startGlobalTimer(timeout);
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
    assert(!timer.scheduled);
    assert(now <= timer.due);
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
      assert(list[0] === timer);
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
      assert(index > -1);
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
      globalThis.console.warn(
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

  function setInterval(cb, delay = 0, ...args) {
    checkBigInt(delay);
    // @ts-ignore
    checkThis(this);
    return setTimer(cb, delay, args, true);
  }

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

  function clearInterval(id = 0) {
    checkBigInt(id);
    if (id === 0) {
      return;
    }
    clearTimer(id);
  }

  // MIN_READ is the minimum ArrayBuffer size passed to a read call by
  // buffer.ReadFrom. As long as the Buffer has at least MIN_READ bytes beyond
  // what is required to hold the contents of r, readFrom() will not grow the
  // underlying buffer.
  const MIN_READ = 512;
  const MAX_SIZE = 2 ** 32 - 2;

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

  class Buffer {
    #buf = undefined; // contents are the bytes buf[off : len(buf)]
    #off = 0; // read at buf[off], write at buf[buf.byteLength]

    constructor(ab) {
      if (ab == null) {
        this.#buf = new Uint8Array(0);
        return;
      }

      this.#buf = new Uint8Array(ab);
    }

    bytes() {
      return this.#buf.subarray(this.#off);
    }

    toString() {
      const decoder = new TextDecoder();
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

    #tryGrowByReslice = (n) => {
      const l = this.#buf.byteLength;
      if (n <= this.capacity - l) {
        this.#reslice(l + n);
        return l;
      }
      return -1;
    };

    #reslice = (len) => {
      assert(len <= this.#buf.buffer.byteLength);
      this.#buf = new Uint8Array(this.#buf.buffer, 0, len);
    };

    readSync(p) {
      if (this.empty()) {
        // Buffer is empty, reset to recover space.
        this.reset();
        if (p.byteLength === 0) {
          // this edge case is tested in 'bufferReadEmptyAtEOF' test
          return 0;
        }
        return EOF;
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

    #grow = (n) => {
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
        throw new Error("The buffer cannot be grown beyond the maximum size.");
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
          if (nread === EOF) {
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
          if (nread === EOF) {
            return n;
          }
          this.#reslice(i + nread);
          n += nread;
        } catch (e) {
          return n;
        }
      }
    }
  }

  async function readAll(r) {
    const buf = new Buffer();
    await buf.readFrom(r);
    return buf.bytes();
  }

  function readAllSync(r) {
    const buf = new Buffer();
    buf.readFromSync(r);
    return buf.bytes();
  }

  async function writeAll(w, arr) {
    let nwritten = 0;
    while (nwritten < arr.length) {
      nwritten += await w.write(arr.subarray(nwritten));
    }
  }

  function writeAllSync(w, arr) {
    let nwritten = 0;
    while (nwritten < arr.length) {
      nwritten += w.writeSync(arr.subarray(nwritten));
    }
  }

  function chmodSync(path, mode) {
    sendSyncJson("op_chmod", { path, mode });
  }

  async function chmod(path, mode) {
    await sendAsyncJson("op_chmod", { path, mode });
  }

  function chownSync(path, uid, gid) {
    sendSyncJson("op_chown", { path, uid, gid });
  }

  async function chown(path, uid, gid) {
    await sendAsyncJson("op_chown", { path, uid, gid });
  }

  function copyFileSync(fromPath, toPath) {
    sendSyncJson("op_copy_file", { from: fromPath, to: toPath });
  }

  async function copyFile(fromPath, toPath) {
    await sendAsyncJson("op_copy_file", { from: fromPath, to: toPath });
  }

  function cwd() {
    return sendSyncJson("op_cwd");
  }

  function chdir(directory) {
    sendSyncJson("op_chdir", { directory });
  }

  const netOps = (function () {
    function shutdown(rid, how) {
      sendSyncJson("op_shutdown", { rid, how });
    }

    function accept(rid, transport) {
      return sendAsyncJson("op_accept", { rid, transport });
    }

    function listen(args) {
      return sendSyncJson("op_listen", args);
    }

    function connect(args) {
      return sendAsyncJson("op_connect", args);
    }

    function receive(rid, transport, zeroCopy) {
      return sendAsyncJson("op_receive", { rid, transport }, zeroCopy);
    }

    async function send(args, zeroCopy) {
      await sendAsyncJson("op_send", args, zeroCopy);
    }

    return {
      accept,
      shutdown,
      listen,
      connect,
      receive,
      send,
    };
  })();
  class ConnImpl {
    constructor(rid, remoteAddr, localAddr) {
      this.rid = rid;
      this.remoteAddr = remoteAddr;
      this.localAddr = localAddr;
    }

    write(p) {
      return write(this.rid, p);
    }

    read(p) {
      return read(this.rid, p);
    }

    close() {
      close(this.rid);
    }

    closeRead() {
      netOps.shutdown(this.rid, netOps.ShutdownMode.Read);
    }

    closeWrite() {
      netOps.shutdown(this.rid, netOps.ShutdownMode.Write);
    }
  }

  class ListenerImpl {
    constructor(rid, addr) {
      this.rid = rid;
      this.addr = addr;
    }

    async accept() {
      const res = await netOps.accept(this.rid, this.addr.transport);
      return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
    }

    close() {
      close(this.rid);
    }

    async *[Symbol.asyncIterator]() {
      while (true) {
        try {
          yield await this.accept();
        } catch (error) {
          if (error instanceof errors.BadResource) {
            break;
          }
          throw error;
        }
      }
    }
  }

  class DatagramImpl {
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
      close(this.rid);
    }

    async *[Symbol.asyncIterator]() {
      while (true) {
        try {
          yield await this.receive();
        } catch (error) {
          if (error instanceof errors.BadResource) {
            break;
          }
          throw error;
        }
      }
    }
  }

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

  function linkSync(oldpath, newpath) {
    sendSyncJson("op_link", { oldpath, newpath });
  }

  async function link(oldpath, newpath) {
    await sendAsyncJson("op_link", { oldpath, newpath });
  }

  function formatDiagnostics(items) {
    return sendSyncJson("op_format_diagnostic", { items });
  }

  function applySourceMap(location) {
    const { fileName, lineNumber, columnNumber } = location;
    const res = sendSyncJson("op_apply_source_map", {
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

  class FsEvents {
    constructor(paths, options) {
      const { recursive } = options;
      this.rid = sendSyncJson("op_fs_events_open", { recursive, paths });
    }

    next() {
      return sendAsyncJson("op_fs_events_poll", {
        rid: this.rid,
      });
    }

    return(value) {
      close(this.rid);
      return Promise.resolve({ value, done: true });
    }

    [Symbol.asyncIterator]() {
      return this;
    }
  }

  function fsEvents(paths, options = { recursive: true }) {
    return new FsEvents(Array.isArray(paths) ? paths : [paths], options);
  }

  function makeTempDirSync(options = {}) {
    return sendSyncJson("op_make_temp_dir", options);
  }

  function makeTempDir(options = {}) {
    return sendAsyncJson("op_make_temp_dir", options);
  }

  function makeTempFileSync(options = {}) {
    return sendSyncJson("op_make_temp_file", options);
  }

  function makeTempFile(options = {}) {
    return sendAsyncJson("op_make_temp_file", options);
  }

  class PermissionStatus {
    constructor(state) {
      this.state = state;
    }
  }

  class Permissions {
    query(desc) {
      const state = sendSyncJson("op_query_permission", desc).state;
      return Promise.resolve(new PermissionStatus(state));
    }

    revoke(desc) {
      const state = sendSyncJson("op_revoke_permission", desc).state;
      return Promise.resolve(new PermissionStatus(state));
    }

    request(desc) {
      const state = sendSyncJson("op_request_permission", desc).state;
      return Promise.resolve(new PermissionStatus(state));
    }
  }

  const permissions = new Permissions();

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
    sendSyncJson("op_mkdir", mkdirArgs(path, options));
  }

  async function mkdir(path, options) {
    await sendAsyncJson("op_mkdir", mkdirArgs(path, options));
  }

  class FileInfoImpl {
    #isFile = false;
    #isDirectory = false;
    #isSymlink = false;

    /* @internal */
    constructor(res) {
      const isUnix = build.os === "mac" || build.os === "linux";
      const modified = res.modified;
      const accessed = res.accessed;
      const created = res.created;
      const name = res.name;
      // Unix only
      const { dev, ino, mode, nlink, uid, gid, rdev, blksize, blocks } = res;

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

    isFile() {
      return this.#isFile;
    }

    isDirectory() {
      return this.#isDirectory;
    }

    isSymlink() {
      return this.#isSymlink;
    }
  }

  function readdirRes(response) {
    return response.entries.map((statRes) => {
      return new FileInfoImpl(statRes);
    });
  }

  function readdirSync(path) {
    return readdirRes(sendSyncJson("op_read_dir", { path }));
  }

  async function readdir(path) {
    return readdirRes(await sendAsyncJson("op_read_dir", { path }));
  }

  function readFileSync(path) {
    const file = openSync(path);
    const contents = readAllSync(file);
    file.close();
    return contents;
  }

  async function readFile(path) {
    const file = await open(path);
    const contents = await readAll(file);
    file.close();
    return contents;
  }

  function readlinkSync(path) {
    return sendSyncJson("op_read_link", { path });
  }

  function readlink(path) {
    return sendAsyncJson("op_read_link", { path });
  }

  function realpathSync(path) {
    return sendSyncJson("op_realpath", { path });
  }

  function realpath(path) {
    return sendAsyncJson("op_realpath", { path });
  }

  function removeSync(path, options = {}) {
    sendSyncJson("op_remove", { path, recursive: !!options.recursive });
  }

  async function remove(path, options = {}) {
    await sendAsyncJson("op_remove", { path, recursive: !!options.recursive });
  }

  function renameSync(oldpath, newpath) {
    sendSyncJson("op_rename", { oldpath, newpath });
  }

  async function rename(oldpath, newpath) {
    await sendAsyncJson("op_rename", { oldpath, newpath });
  }

  async function lstat(path) {
    const res = await sendAsyncJson("op_stat", {
      path,
      lstat: true,
    });
    return new FileInfoImpl(res);
  }

  function lstatSync(path) {
    const res = sendSyncJson("op_stat", {
      path,
      lstat: true,
    });
    return new FileInfoImpl(res);
  }

  async function stat(path) {
    const res = await sendAsyncJson("op_stat", {
      path,
      lstat: false,
    });
    return new FileInfoImpl(res);
  }

  function statSync(path) {
    const res = sendSyncJson("op_stat", {
      path,
      lstat: false,
    });
    return new FileInfoImpl(res);
  }

  function symlinkSync(oldpath, newpath, type) {
    if (build.os === "win" && type) {
      return notImplemented();
    }
    sendSyncJson("op_symlink", { oldpath, newpath });
  }

  async function symlink(oldpath, newpath, type) {
    if (build.os === "win" && type) {
      return notImplemented();
    }
    await sendAsyncJson("op_symlink", { oldpath, newpath });
  }

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
    sendSyncJson("op_truncate", { path, len: coerceLen(len) });
  }

  async function truncate(path, len) {
    await sendAsyncJson("op_truncate", { path, len: coerceLen(len) });
  }

  function umask(mask) {
    return sendSyncJson("op_umask", { mask });
  }

  function toSecondsFromEpoch(v) {
    return v instanceof Date ? Math.trunc(v.valueOf() / 1000) : v;
  }

  function utimeSync(path, atime, mtime) {
    sendSyncJson("op_utime", {
      path,
      // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
      atime: toSecondsFromEpoch(atime),
      mtime: toSecondsFromEpoch(mtime),
    });
  }

  async function utime(path, atime, mtime) {
    await sendAsyncJson("op_utime", {
      path,
      // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
      atime: toSecondsFromEpoch(atime),
      mtime: toSecondsFromEpoch(mtime),
    });
  }

  function isatty(rid) {
    return sendSyncJson("op_isatty", { rid });
  }

  function setRaw(rid, mode) {
    sendSyncJson("op_set_raw", {
      rid,
      mode,
    });
  }

  function writeFileSync(path, data, options = {}) {
    if (options.create !== undefined) {
      const create = !!options.create;
      if (!create) {
        // verify that file exists
        statSync(path);
      }
    }

    const openMode = !!options.append ? "a" : "w";
    const file = openSync(path, openMode);

    if (
      options.mode !== undefined &&
      options.mode !== null &&
      build.os !== "win"
    ) {
      chmodSync(path, options.mode);
    }

    writeAllSync(file, data);
    file.close();
  }

  async function writeFile(path, data, options = {}) {
    if (options.create !== undefined) {
      const create = !!options.create;
      if (!create) {
        // verify that file exists
        await stat(path);
      }
    }

    const openMode = !!options.append ? "a" : "w";
    const file = await open(path, openMode);

    if (
      options.mode !== undefined &&
      options.mode !== null &&
      build.os !== "win"
    ) {
      await chmod(path, options.mode);
    }

    await writeAll(file, data);
    file.close();
  }

  function runStatusOp(rid) {
    return sendAsyncJson("op_run_status", { rid });
  }

  function kill(pid, signo) {
    if (build.os === "win") {
      throw new Error("Not yet implemented");
    }
    sendSyncJson("op_kill", { pid, signo });
  }

  function runOp(request) {
    assert(request.cmd.length > 0);
    return sendSyncJson("op_run", request);
  }

  async function runStatus(rid) {
    const res = await runStatusOp(rid);

    if (res.gotSignal) {
      const signal = res.exitSignal;
      return { signal, success: false };
    } else {
      const code = res.exitCode;
      return { code, success: code === 0 };
    }
  }

  class Process {
    constructor(res) {
      this.rid = res.rid;
      this.pid = res.pid;

      if (res.stdinRid && res.stdinRid > 0) {
        this.stdin = new File(res.stdinRid);
      }

      if (res.stdoutRid && res.stdoutRid > 0) {
        this.stdout = new File(res.stdoutRid);
      }

      if (res.stderrRid && res.stderrRid > 0) {
        this.stderr = new File(res.stderrRid);
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
        return await readAll(this.stdout);
      } finally {
        this.stdout.close();
      }
    }

    async stderrOutput() {
      if (!this.stderr) {
        throw new Error("Process.stderrOutput: stderr is undefined");
      }
      try {
        return await readAll(this.stderr);
      } finally {
        this.stderr.close();
      }
    }

    close() {
      close(this.rid);
    }

    kill(signo) {
      kill(this.pid, signo);
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
    const res = runOp({
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

  class PluginOpImpl {
    #opId = 0;

    constructor(opId) {
      this.#opId = opId;
    }

    dispatch(control, zeroCopy) {
      return core.dispatch(this.#opId, control, zeroCopy);
    }

    setAsyncHandler(handler) {
      core.setAsyncHandler(this.#opId, handler);
    }
  }

  class PluginImpl {
    #ops = {};

    constructor(_rid, ops) {
      for (const op in ops) {
        this.#ops[op] = new PluginOpImpl(ops[op]);
      }
    }

    get ops() {
      return Object.assign({}, this.#ops);
    }
  }

  function openPlugin(filename) {
    const response = openPluginOp(filename);
    return new PluginImpl(response.rid, response.ops);
  }

  const tlsOps = (function () {
    function connectTLS(args) {
      return sendAsyncJson("op_connect_tls", args);
    }

    function acceptTLS(rid) {
      return sendAsyncJson("op_accept_tls", { rid });
    }

    function listenTLS(args) {
      return sendSyncJson("op_listen_tls", args);
    }
    return {
      listenTLS,
      acceptTLS,
      connectTLS,
    };
  })();

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
    return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
  }

  class TLSListenerImpl extends ListenerImpl {
    async accept() {
      const res = await tlsOps.acceptTLS(this.rid);
      return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
    }
  }

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

  const runtimeCompilerOps = (function () {
    function compile(request) {
      return sendAsyncJson("op_compile", request);
    }

    function transpile(request) {
      return sendAsyncJson("op_transpile", request);
    }

    return {
      compile,
      transpile,
    };
  })();

  function checkRelative(specifier) {
    return specifier.match(/^([\.\/\\]|https?:\/{2}|file:\/{2})/)
      ? specifier
      : `./${specifier}`;
  }

  async function transpileOnly(sources, options = {}) {
    log("Deno.transpileOnly", { sources: Object.keys(sources), options });
    const payload = {
      sources,
      options: JSON.stringify(options),
    };
    const result = await runtimeCompilerOps.transpile(payload);
    return JSON.parse(result);
  }

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

  const RED_FAILED = "FAILED";
  const GREEN_OK = "ok";
  const YELLOW_IGNORED = "ignored";
  const disabledConsole = new Console(() => {});

  function delay(n) {
    return new Promise((resolve, _) => {
      setTimeout(resolve, n);
    });
  }

  function formatDuration(time = 0) {
    const timeStr = `(${time}ms)`;
    return timeStr;
  }

  // Wrap test function in additional assertion that makes sure
  // the test case does not leak async "ops" - ie. number of async
  // completed ops after the test is the same as number of dispatched
  // ops. Note that "unref" ops are ignored since in nature that are
  // optional.
  function assertOps(fn) {
    return async function asyncOpSanitizer() {
      const pre = metrics();
      await fn();
      // Defer until next event loop turn - that way timeouts and intervals
      // cleared can actually be removed from resource table, otherwise
      // false positives may occur (https://github.com/denoland/deno/issues/4591)
      await delay(0);
      const post = metrics();
      // We're checking diff because one might spawn HTTP server in the background
      // that will be a pending async op before test starts.
      const dispatchedDiff = post.opsDispatchedAsync - pre.opsDispatchedAsync;
      const completedDiff = post.opsCompletedAsync - pre.opsCompletedAsync;
      assert(
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
      const pre = resources();
      await fn();
      const post = resources();

      const preStr = JSON.stringify(pre, null, 2);
      const postStr = JSON.stringify(post, null, 2);
      const msg = `Test case is leaking resources.
  Before: ${preStr}
  After: ${postStr}`;
      assert(preStr === postStr, msg);
    };
  }

  const TEST_REGISTRY = [];

  function test(t, fn) {
    let testDef;

    if (typeof t === "string") {
      if (!fn || typeof fn != "function") {
        throw new TypeError("Missing test function");
      }
      if (!t) {
        throw new TypeError("The test name can't be empty");
      }
      testDef = { fn, name: t, ignore: false };
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

  function testLog(msg, noNewLine = false) {
    if (!noNewLine) {
      msg += "\n";
    }

    // Using `stdout` here because it doesn't force new lines
    // compared to `console.log`; `core.print` on the other hand
    // is line-buffered and doesn't output message without newline
    stdout.writeSync(encoder.encode(msg));
  }

  function reportToConsole(message) {
    if (message.start != null) {
      testLog(`running ${message.start.tests.length} tests`);
    } else if (message.testStart != null) {
      const { name } = message.testStart;

      testLog(`test ${name} ... `, true);
      return;
    } else if (message.testEnd != null) {
      switch (message.testEnd.status) {
        case "passed":
          testLog(`${GREEN_OK} ${formatDuration(message.testEnd.duration)}`);
          break;
        case "failed":
          testLog(`${RED_FAILED} ${formatDuration(message.testEnd.duration)}`);
          break;
        case "ignored":
          testLog(
            `${YELLOW_IGNORED} ${formatDuration(message.testEnd.duration)}`
          );
          break;
      }
    } else if (message.end != null) {
      const failures = message.end.results.filter((m) => m.error != null);
      if (failures.length > 0) {
        testLog(`\nfailures:\n`);

        for (const { name, error } of failures) {
          testLog(name);
          testLog(stringifyArgs([error]));
          testLog("");
        }

        testLog(`failures:\n`);

        for (const { name } of failures) {
          testLog(`\t${name}`);
        }
      }
      testLog(
        `\ntest result: ${message.end.failed ? RED_FAILED : GREEN_OK}. ` +
          `${message.end.passed} passed; ${message.end.failed} failed; ` +
          `${message.end.ignored} ignored; ${message.end.measured} measured; ` +
          `${message.end.filtered} filtered out ` +
          `${formatDuration(message.end.duration)}\n`
      );
    }
  }

  exposeForTest("reportToConsole", reportToConsole);

  // TODO: already implements AsyncGenerator<RunTestsMessage>, but add as "implements to class"
  // TODO: implements PromiseLike<RunTestsEndResult>
  class TestApi {
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
      exit(1);
    }

    return endMsg;
  }

  const symbols = {
    internal: internalSymbol,
    customInspect,
  };

  const DenoNs = (function () {
    // Public deno module.

    return {
      test,
      runTests,
      chmod,
      chmodSync,
      chown,
      compile,
      bundle,
      transpileOnly,
      chownSync,
      copyFile,
      copyFileSync,
      chdir,
      cwd,
      connect,
      listen,
      applySourceMap,
      formatDiagnostics,
      run,
      Process,
      kill,
      shutdown: netOps.shutdown,
      Buffer,
      build,
      connectTLS,
      EOF,
      listenTLS,
      openPlugin,
      makeTempDirSync,
      umask,
      makeTempDir,
      isatty,
      setRaw,
      makeTempFileSync,
      makeTempFile,
      remove,
      removeSync,
      utime,
      utimeSync,
      truncate,
      truncateSync,
      writeFile,
      writeFileSync,
      readlink,
      rename,
      renameSync,
      statSync,
      stat,
      lstat,
      lstatSync,
      readlinkSync,
      mkdir,
      mkdirSync,
      readAll,
      fsEvents,
      copy,
      symlink,
      symlinkSync,
      link,
      realpath,
      realpathSync,
      linkSync,
      toAsyncIterator,
      Permissions,
      readFile,
      readFileSync,
      PermissionStatus,
      permissions,
      readdir,
      readdirSync,
      readAllSync,
      writeAll,
      writeAllSync,
      inspect,
      read,
      readSync,
      write,
      writeSync,
      errors,
      version,
      core,
      dir,
      env,
      exit,
      metrics,
      execPath,
      hostname,
      loadavg,
      osRelease,
      File,
      open,
      openSync,
      create,
      createSync,
      stdin,
      stdout,
      stderr,
      seek,
      seekSync,
      signal,
      signals,
      Signal,
      resources,
      close,
      SignalStream,
      symbols,
      [symbols.internal]: internalObject,
    };
  })();

  let windowIsClosing = false;

  function windowClose() {
    if (!windowIsClosing) {
      windowIsClosing = true;
      // Push a macrotask to exit after a promise resolve.
      // This is not perfect, but should be fine for first pass.
      Promise.resolve().then(() =>
        setTimeout.call(
          null,
          () => {
            // This should be fine, since only Window/MainWorker has .close()
            exit(0);
          },
          0
        )
      );
    }
  }

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
        requiredArguments(
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

  exposeForTest("DomIterableMixin", DomIterableMixin);

  const dataSymbol = Symbol("data");

  class FormDataBase {
    [dataSymbol] = [];

    append(name, value, filename) {
      requiredArguments("FormData.append", arguments.length, 2);
      name = String(name);
      if (value instanceof DomFileImpl) {
        this[dataSymbol].push([name, value]);
      } else if (value instanceof DenoBlob) {
        const dfile = new DomFileImpl([value], filename || name, {
          type: value.type,
        });
        this[dataSymbol].push([name, dfile]);
      } else {
        this[dataSymbol].push([name, String(value)]);
      }
    }

    delete(name) {
      requiredArguments("FormData.delete", arguments.length, 1);
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
      requiredArguments("FormData.getAll", arguments.length, 1);
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
      requiredArguments("FormData.get", arguments.length, 1);
      name = String(name);
      for (const entry of this[dataSymbol]) {
        if (entry[0] === name) {
          return entry[1];
        }
      }

      return null;
    }

    has(name) {
      requiredArguments("FormData.has", arguments.length, 1);
      name = String(name);
      return this[dataSymbol].some((entry) => entry[0] === name);
    }

    set(name, value, filename) {
      requiredArguments("FormData.set", arguments.length, 2);
      name = String(name);

      // If there are any entries in the context object’s entry list whose name
      // is name, replace the first such entry with entry and remove the others
      let found = false;
      let i = 0;
      while (i < this[dataSymbol].length) {
        if (this[dataSymbol][i][0] === name) {
          if (!found) {
            if (value instanceof DomFileImpl) {
              this[dataSymbol][i][1] = value;
            } else if (value instanceof DenoBlob) {
              const dfile = new DomFileImpl([value], filename || name, {
                type: value.type,
              });
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
        if (value instanceof DomFileImpl) {
          this[dataSymbol].push([name, value]);
        } else if (value instanceof DenoBlob) {
          const dfile = new DomFileImpl([value], filename || name, {
            type: value.type,
          });
          this[dataSymbol].push([name, dfile]);
        } else {
          this[dataSymbol].push([name, String(value)]);
        }
      }
    }

    get [Symbol.toStringTag]() {
      return "FormData";
    }
  }

  class FormDataImpl extends DomIterableMixin(FormDataBase, dataSymbol) {}

  // From node-fetch
  // Copyright (c) 2016 David Frank. MIT License.
  const invalidTokenRegex = /[^\^_`a-zA-Z\-0-9!#$%&'*+.|~]/;
  const invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  function isHeaders(value) {
    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    return value instanceof Headers;
  }

  const headerMap = Symbol("header map");

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

  // ref: https://fetch.spec.whatwg.org/#dom-headers
  class HeadersBase {
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
            requiredArguments(
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

    [customInspect]() {
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
      requiredArguments("Headers.append", arguments.length, 2);
      const [newname, newvalue] = normalizeParams(name, value);
      validateName(newname);
      validateValue(newvalue);
      const v = this[headerMap].get(newname);
      const str = v ? `${v}, ${newvalue}` : newvalue;
      this[headerMap].set(newname, str);
    }

    delete(name) {
      requiredArguments("Headers.delete", arguments.length, 1);
      const [newname] = normalizeParams(name);
      validateName(newname);
      this[headerMap].delete(newname);
    }

    get(name) {
      requiredArguments("Headers.get", arguments.length, 1);
      const [newname] = normalizeParams(name);
      validateName(newname);
      const value = this[headerMap].get(newname);
      return value || null;
    }

    has(name) {
      requiredArguments("Headers.has", arguments.length, 1);
      const [newname] = normalizeParams(name);
      validateName(newname);
      return this[headerMap].has(newname);
    }

    set(name, value) {
      requiredArguments("Headers.set", arguments.length, 2);
      const [newname, newvalue] = normalizeParams(name, value);
      validateName(newname);
      validateValue(newvalue);
      this[headerMap].set(newname, newvalue);
    }

    get [Symbol.toStringTag]() {
      return "Headers";
    }
  }

  // @internal
  class HeadersImpl extends DomIterableMixin(HeadersBase, headerMap) {}

  const request = (function () {
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
      } else if (bodySource instanceof ReadableStream) {
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

    const BodyUsedError =
      "Failed to execute 'clone' on 'Body': body is already used";

    class Body {
      constructor(_bodySource, contentType) {
        validateBodyType(this, _bodySource);
        this._bodySource = _bodySource;
        this.contentType = contentType;
        this._stream = null;
      }

      get body() {
        if (this._stream) {
          return this._stream;
        }

        if (this._bodySource instanceof ReadableStream) {
          // @ts-ignore
          this._stream = this._bodySource;
        }
        if (typeof this._bodySource === "string") {
          const bodySource = this._bodySource;
          this._stream = new ReadableStream({
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
            throw new TypeError("multipart/form-data must provide a boundary");
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
            const partContentType = headers.get("content-type") || "text/plain";
            // TODO: custom charset encoding (needs TextEncoder support)
            // const contentTypeCharset =
            //   getHeaderValueParams(partContentType).get("charset") || "";
            if (!hasHeaderValueOf(contentDisposition, "form-data")) {
              continue; // Skip, might not be form-data
            }
            const dispositionParams = getHeaderValueParams(contentDisposition);
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
        } else if (this._bodySource instanceof ReadableStream) {
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
    }

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

    class Request extends Body {
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
        } else if (typeof input === "object" && "body" in input && input.body) {
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
          ["omit", "same-origin", "include"].indexOf(init.credentials) !== -1
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
    }

    return { Request };
  })();

  const fetchTypes = (function () {
    function opFetch(args, body) {
      let zeroCopy = undefined;
      if (body) {
        zeroCopy = new Uint8Array(
          body.buffer,
          body.byteOffset,
          body.byteLength
        );
      }

      return sendAsyncJson("op_fetch", args, zeroCopy);
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

    class Body {
      #bodyUsed = false;
      #bodyPromise = null;
      #data = null;
      #rid = 0;

      constructor(rid, contentType) {
        this.#rid = rid;
        this.body = this;
        this.contentType = contentType;
      }

      #bodyBuffer = async () => {
        assert(this.#bodyPromise == null);
        const buf = new Buffer();
        try {
          const nread = await buf.readFrom(this);
          const ui8 = buf.bytes();
          assert(ui8.byteLength === nread);
          this.#data = ui8.buffer.slice(ui8.byteOffset, ui8.byteOffset + nread);
          assert(this.#data.byteLength === nread);
        } finally {
          this.close();
        }

        return this.#data;
      };

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
        return new DenoBlob([arrayBuffer], {
          type: this.contentType,
        });
      }

      // ref: https://fetch.spec.whatwg.org/#body-mixin
      async formData() {
        const formData = new FormData();
        const enc = new TextEncoder();
        if (hasHeaderValueOf(this.contentType, "multipart/form-data")) {
          const params = getHeaderValueParams(this.contentType);
          if (!params.has("boundary")) {
            // TypeError is required by spec
            throw new TypeError("multipart/form-data must provide a boundary");
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
            const partContentType = headers.get("content-type") || "text/plain";
            // TODO: custom charset encoding (needs TextEncoder support)
            // const contentTypeCharset =
            //   getHeaderValueParams(partContentType).get("charset") || "";
            if (!hasHeaderValueOf(contentDisposition, "form-data")) {
              continue; // Skip, might not be form-data
            }
            const dispositionParams = getHeaderValueParams(contentDisposition);
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

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      async json() {
        const text = await this.text();
        return JSON.parse(text);
      }

      async text() {
        const ab = await this.arrayBuffer();
        const decoder = new TextDecoder("utf-8");
        return decoder.decode(ab);
      }

      read(p) {
        this.#bodyUsed = true;
        return read(this.#rid, p);
      }

      close() {
        close(this.#rid);
        return Promise.resolve();
      }

      cancel() {
        return notImplemented();
      }

      getReader() {
        return notImplemented();
      }

      tee() {
        return notImplemented();
      }

      [Symbol.asyncIterator]() {
        return io.toAsyncIterator(this);
      }

      get bodyUsed() {
        return this.#bodyUsed;
      }

      pipeThrough(_, _options) {
        return notImplemented();
      }

      pipeTo(_dest, _options) {
        return notImplemented();
      }
    }

    class Response {
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
        this.trailer = createResolvable();
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
              if (["set-cookie", "set-cookie2"].includes(h[0].toLowerCase())) {
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

      #bodyViewable = () => {
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

      return opFetch(args, body);
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
              body = new TextEncoder().encode(init.body);
              contentType = "text/plain;charset=UTF-8";
            } else if (isTypedArray(init.body)) {
              body = init.body;
            } else if (init.body instanceof URLSearchParams) {
              body = new TextEncoder().encode(init.body.toString());
              contentType = "application/x-www-form-urlencoded;charset=UTF-8";
            } else if (init.body instanceof DenoBlob) {
              body = init.body[bytesSymbol];
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
                if (fieldValue instanceof DomFileImpl) {
                  part += `; filename=\"${fieldValue.name}\"`;
                }
                part += "\r\n";
                if (fieldValue instanceof DomFileImpl) {
                  part += `Content-Type: ${
                    fieldValue.type || "application/octet-stream"
                  }\r\n`;
                }
                part += "\r\n";
                if (fieldValue instanceof DomFileImpl) {
                  part += new TextDecoder().decode(fieldValue[bytesSymbol]);
                } else {
                  part += fieldValue;
                }
                payload += part;
              }
              payload += `\r\n--${boundary}--`;
              body = new TextEncoder().encode(payload);
              contentType = "multipart/form-data; boundary=" + boundary;
            } else {
              // TODO: ReadableStream
              notImplemented();
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
          close(fetchResponse.bodyRid);
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
      throw notImplemented();
    }

    return {
      fetch,
      Response,
    };
  })();

  class Performance {
    now() {
      const res = now();
      return res.seconds * 1e3 + res.subsecNanos / 1e6;
    }
  }

  function createWorker(specifier, hasSourceCode, sourceCode, name) {
    return sendSyncJson("op_create_worker", {
      specifier,
      hasSourceCode,
      sourceCode,
      name,
    });
  }

  function hostTerminateWorker(id) {
    sendSyncJson("op_host_terminate_worker", { id });
  }

  function hostPostMessage(id, data) {
    sendSyncJson("op_host_post_message", { id }, data);
  }

  function hostGetMessage(id) {
    return sendAsyncJson("op_host_get_message", { id });
  }

  /*
  import { blobURLMap } from "./web/url.ts";
  */
  class MessageEvent extends EventImpl {
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
  }

  class ErrorEvent extends EventImpl {
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
  }

  function encodeMessage(data) {
    const dataJson = JSON.stringify(data);
    return encoder.encode(dataJson);
  }

  function decodeMessage(dataIntArray) {
    const dataJson = decoder.decode(dataIntArray);
    return JSON.parse(dataJson);
  }

  class WorkerImpl extends EventTargetImpl {
    #id = 0;
    #name = "";
    #terminated = false;

    constructor(specifier, options) {
      super();
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

      const { id } = createWorker(
        specifier,
        hasSourceCode,
        sourceCode,
        options?.name
      );
      this.#id = id;
      this.#poll();
    }

    #handleMessage = (msgData) => {
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

    #handleError = (e) => {
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

    #poll = async () => {
      while (!this.#terminated) {
        const event = await hostGetMessage(this.#id);

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
          log(`Host got "close" message from worker: ${this.#name}`);
          this.#terminated = true;
          return;
        }

        throw new Error(`Unknown worker event: "${type}"`);
      }
    };

    postMessage(message, transferOrOptions) {
      if (transferOrOptions) {
        throw new Error(
          "Not yet implemented: `transfer` and `options` are not supported."
        );
      }

      if (this.#terminated) {
        return;
      }

      hostPostMessage(this.#id, encodeMessage(message));
    }

    terminate() {
      if (!this.#terminated) {
        this.#terminated = true;
        hostTerminateWorker(this.#id);
      }
    }
  }

  // https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope
  const windowOrWorkerGlobalScopeMethods = {
    atob: writable(atob),
    btoa: writable(btoa),
    clearInterval: writable(clearInterval),
    clearTimeout: writable(clearTimeout),
    fetch: writable(fetchTypes.fetch),
    // queueMicrotask is bound in Rust
    setInterval: writable(setInterval),
    setTimeout: writable(setTimeout),
  };

  // Other properties shared between WindowScope and WorkerGlobalScope
  const windowOrWorkerGlobalScopeProperties = {
    console: writable(new Console(core.print)),
    Blob: nonEnumerable(DenoBlob),
    File: nonEnumerable(DomFileImpl),
    CustomEvent: nonEnumerable(CustomEventImpl),
    DOMException: nonEnumerable(DOMException),
    Event: nonEnumerable(EventImpl),
    EventTarget: nonEnumerable(EventTargetImpl),
    URL: nonEnumerable(URLImpl),
    URLSearchParams: nonEnumerable(URLSearchParamsImpl),
    Headers: nonEnumerable(HeadersImpl),
    FormData: nonEnumerable(FormDataImpl),
    TextEncoder: nonEnumerable(TextEncoder),
    TextDecoder: nonEnumerable(TextDecoder),
    ReadableStream: nonEnumerable(streams.ReadableStream),
    Request: nonEnumerable(request.Request),
    Response: nonEnumerable(fetchTypes.Response),
    performance: writable(new Performance()),
    Worker: nonEnumerable(WorkerImpl),
  };

  function setEventTargetData(value) {
    eventTargetData.set(value, getDefaultTargetData());
  }

  const eventTargetProperties = {
    addEventListener: readOnly(EventTargetImpl.prototype.addEventListener),
    dispatchEvent: readOnly(EventTargetImpl.prototype.dispatchEvent),
    removeEventListener: readOnly(
      EventTargetImpl.prototype.removeEventListener
    ),
  };

  const mainRuntimeGlobalProperties = {
    window: readOnly(globalThis),
    self: readOnly(globalThis),
    crypto: readOnly(csprng),
    // TODO(bartlomieju): from MDN docs (https://developer.mozilla.org/en-US/docs/Web/API/WorkerGlobalScope)
    // it seems those two properties should be available to workers as well
    onload: writable(null),
    onunload: writable(null),
    close: writable(windowClose),
    closed: getterOnly(() => windowIsClosing),
  };

  let hasBootstrapped = false;

  function bootstrapMainRuntimeFn() {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }
    log("bootstrapMainRuntime");
    hasBootstrapped = true;
    Object.defineProperties(globalThis, windowOrWorkerGlobalScopeMethods);
    Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
    Object.defineProperties(globalThis, eventTargetProperties);
    Object.defineProperties(globalThis, mainRuntimeGlobalProperties);
    setEventTargetData(globalThis);
    // Registers the handler for window.onload function.
    globalThis.addEventListener("load", (e) => {
      const { onload } = globalThis;
      if (typeof onload === "function") {
        onload(e);
      }
    });
    // Registers the handler for window.onunload function.
    globalThis.addEventListener("unload", (e) => {
      const { onunload } = globalThis;
      if (typeof onunload === "function") {
        onunload(e);
      }
    });

    const s = start();

    const location = new LocationImpl(s.location);
    immutableDefine(globalThis, "location", location);
    Object.freeze(globalThis.location);

    Object.defineProperties(DenoNs, {
      pid: readOnly(s.pid),
      noColor: readOnly(s.noColor),
      args: readOnly(Object.freeze(s.args)),
    });
    // Setup `Deno` global - we're actually overriding already
    // existing global `Deno` with `Deno` namespace from "./deno.ts".
    immutableDefine(globalThis, "Deno", DenoNs);
    Object.freeze(globalThis.Deno);
    Object.freeze(globalThis.Deno.core);
    Object.freeze(globalThis.Deno.core.sharedQueue);
    setSignals();

    log("cwd", s.cwd);
    log("args", Deno.args);

    if (s.repl) {
      replLoop();
    }
  }

  // TODO(bartlomieju): remove these funtions
  // Stuff for workers
  const onmessage = () => {};
  const onerror = () => {};

  function postMessage(data) {
    const dataJson = JSON.stringify(data);
    const dataIntArray = encoder.encode(dataJson);
    webWorkerOps.postMessage(dataIntArray);
  }

  let isClosing = false;

  function closeWorker() {
    if (isClosing) {
      return;
    }

    isClosing = true;
    webWorkerOps.close();
  }

  async function workerMessageRecvCallback(data) {
    const msgEvent = new MessageEvent("message", {
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

      const errorEvent = new ErrorEvent("error", {
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

  const workerRuntimeGlobalProperties = {
    self: readOnly(globalThis),
    onmessage: writable(onmessage),
    onerror: writable(onerror),
    // TODO: should be readonly?
    close: nonEnumerable(closeWorker),
    postMessage: writable(postMessage),
    workerMessageRecvCallback: nonEnumerable(workerMessageRecvCallback),
  };

  function bootstrapWorkerRuntimeFn(name, internalName) {
    if (hasBootstrapped) {
      throw new Error("Worker runtime already bootstrapped");
    }
    log("bootstrapWorkerRuntime");
    hasBootstrapped = true;
    Object.defineProperties(globalThis, windowOrWorkerGlobalScopeMethods);
    Object.defineProperties(globalThis, windowOrWorkerGlobalScopeProperties);
    Object.defineProperties(globalThis, workerRuntimeGlobalProperties);
    Object.defineProperties(globalThis, eventTargetProperties);
    Object.defineProperties(globalThis, { name: readOnly(name) });
    setEventTargetData(globalThis);
    const s = runtime.start(internalName ?? name);

    const location = new LocationImpl(s.location);
    immutableDefine(globalThis, "location", location);
    Object.freeze(globalThis.location);

    // globalThis.Deno is not available in worker scope
    delete globalThis.Deno;
    assert(globalThis.Deno === undefined);
  }

  // Removes the `__proto__` for security reasons.  This intentionally makes
  // Deno non compliant with ECMA-262 Annex B.2.2.1
  delete Object.prototype.__proto__;

  Object.defineProperties(globalThis, {
    bootstrapMainRuntime: {
      value: bootstrapMainRuntimeFn,
      enumerable: false,
      writable: false,
      configurable: false,
    },
    bootstrapWorkerRuntime: {
      value: bootstrapWorkerRuntimeFn,
      enumerable: false,
      writable: false,
      configurable: false,
    },
  });
})();
