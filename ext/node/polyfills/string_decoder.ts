// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

// Logic and comments translated pretty much one-to-one from node's impl
// (https://github.com/nodejs/node/blob/ba06c5c509956dc413f91b755c1c93798bb700d4/src/string_decoder.cc)

// deno-lint-ignore-file prefer-primordials

(function () {
const { core, primordials } = globalThis.__bootstrap;
const bufMod = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { Buffer } = bufMod;
const { MAX_STRING_LENGTH } = bufMod.constants;
const { normalizeEncoding: castEncoding } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
const {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_THIS,
  ERR_UNKNOWN_ENCODING,
  NodeError,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  ArrayBufferIsView,
  ObjectDefineProperties,
  Symbol,
  MathMin,
  DataViewPrototypeGetBuffer,
  DataViewPrototypeGetByteLength,
  DataViewPrototypeGetByteOffset,
  NumberPrototypeToString,
  ObjectPrototypeIsPrototypeOf,
  String,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  StringPrototypeToLowerCase,
  Uint8Array,
} = primordials;
const { isTypedArray } = core;

const ENCODING_UTF8 = 0;
const ENCODING_BASE64 = 1;
const ENCODING_BASE64URL = 2;
const ENCODING_UTF16 = 3;
const ENCODING_ASCII = 4;
const ENCODING_LATIN1 = 5;
const ENCODING_HEX = 6;

function normalizeEncoding(enc) {
  const encoding = castEncoding(enc ?? null);
  if (!encoding) {
    if (typeof enc !== "string" || StringPrototypeToLowerCase(enc) !== "raw") {
      throw new ERR_UNKNOWN_ENCODING(
        enc,
      );
    }
  }
  return String(encoding);
}

function isBufferType(buf) {
  return ObjectPrototypeIsPrototypeOf(Buffer.prototype, buf) &&
    buf.BYTES_PER_ELEMENT;
}

function normalizeBuffer(buf) {
  if (!ArrayBufferIsView(buf)) {
    throw new ERR_INVALID_ARG_TYPE(
      "buf",
      ["Buffer", "TypedArray", "DataView"],
      buf,
    );
  }
  if (isBufferType(buf)) {
    return buf;
  } else {
    const isTA = isTypedArray(buf);
    return Buffer.from(
      new Uint8Array(
        isTA
          ? TypedArrayPrototypeGetBuffer(buf)
          : DataViewPrototypeGetBuffer(buf),
        isTA
          ? TypedArrayPrototypeGetByteOffset(buf)
          : DataViewPrototypeGetByteOffset(buf),
        isTA
          ? TypedArrayPrototypeGetByteLength(buf)
          : DataViewPrototypeGetByteLength(buf),
      ),
    );
  }
}

const maxStringLengthHex = NumberPrototypeToString(MAX_STRING_LENGTH, 16);
function bufferToString(buf, encoding, start, end) {
  const len = (end ?? buf.length) - (start ?? 0);
  if (len > MAX_STRING_LENGTH) {
    throw new NodeError(
      "ERR_STRING_TOO_LONG",
      `Cannot create a string longer than 0x${maxStringLengthHex} characters`,
    );
  }
  return buf.toString(encoding, start, end);
}

const kBufferedBytes = Symbol("bufferedBytes");
const kMissingBytes = Symbol("missingBytes");

function decode(buf) {
  const enc = this.enc;

  let bufIdx = 0;
  let bufEnd = buf.length;

  let prepend = "";
  let rest = "";

  if (
    enc === ENCODING_UTF8 || enc === ENCODING_UTF16 ||
    enc === ENCODING_BASE64 || enc === ENCODING_BASE64URL
  ) {
    if (this[kMissingBytes] > 0) {
      if (enc === ENCODING_UTF8) {
        for (
          let i = 0;
          i < buf.length - bufIdx && i < this[kMissingBytes];
          i++
        ) {
          if ((buf[i] & 0xC0) !== 0x80) {
            this[kMissingBytes] = 0;
            buf.copy(this.lastChar, this[kBufferedBytes], bufIdx, bufIdx + i);
            this[kBufferedBytes] += i;
            bufIdx += i;
            break;
          }
        }
      }

      const bytesToCopy = MathMin(buf.length - bufIdx, this[kMissingBytes]);
      buf.copy(
        this.lastChar,
        this[kBufferedBytes],
        bufIdx,
        bufIdx + bytesToCopy,
      );

      bufIdx += bytesToCopy;

      this[kBufferedBytes] += bytesToCopy;
      this[kMissingBytes] -= bytesToCopy;

      if (this[kMissingBytes] === 0) {
        prepend = bufferToString(
          this.lastChar,
          this.encoding,
          0,
          this[kBufferedBytes],
        );
        this[kBufferedBytes] = 0;
      }
    }

    if (buf.length - bufIdx === 0) {
      rest = prepend.length > 0 ? prepend : "";
      prepend = "";
    } else {
      if (enc === ENCODING_UTF8 && (buf[buf.length - 1] & 0x80)) {
        for (let i = buf.length - 1;; i--) {
          this[kBufferedBytes] += 1;
          if ((buf[i] & 0xC0) === 0x80) {
            if (this[kBufferedBytes] >= 4 || i === 0) {
              this[kBufferedBytes] = 0;
              break;
            }
          } else {
            if ((buf[i] & 0xE0) === 0xC0) {
              this[kMissingBytes] = 2;
            } else if ((buf[i] & 0xF0) === 0xE0) {
              this[kMissingBytes] = 3;
            } else if ((buf[i] & 0xF8) === 0xF0) {
              this[kMissingBytes] = 4;
            } else {
              this[kBufferedBytes] = 0;
              break;
            }

            if (this[kBufferedBytes] >= this[kMissingBytes]) {
              this[kMissingBytes] = 0;
              this[kBufferedBytes] = 0;
            }

            this[kMissingBytes] -= this[kBufferedBytes];
            break;
          }
        }
      } else if (enc === ENCODING_UTF16) {
        if ((buf.length - bufIdx) % 2 === 1) {
          this[kBufferedBytes] = 1;
          this[kMissingBytes] = 1;
        } else if ((buf[buf.length - 1] & 0xFC) === 0xD8) {
          this[kBufferedBytes] = 2;
          this[kMissingBytes] = 2;
        }
      } else if (enc === ENCODING_BASE64 || enc === ENCODING_BASE64URL) {
        this[kBufferedBytes] = (buf.length - bufIdx) % 3;
        if (this[kBufferedBytes] > 0) {
          this[kMissingBytes] = 3 - this[kBufferedBytes];
        }
      }

      if (this[kBufferedBytes] > 0) {
        buf.copy(
          this.lastChar,
          0,
          buf.length - this[kBufferedBytes],
        );
        bufEnd -= this[kBufferedBytes];
      }

      rest = bufferToString(buf, this.encoding, bufIdx, bufEnd);
    }

    if (prepend.length === 0) {
      return rest;
    } else {
      return prepend + rest;
    }
  } else {
    return bufferToString(buf, this.encoding, bufIdx, bufEnd);
  }
}

function flush() {
  const enc = this.enc;

  if (enc === ENCODING_UTF16 && this[kBufferedBytes] % 2 === 1) {
    this[kBufferedBytes] -= 1;
    this[kMissingBytes] -= 1;
  }

  if (this[kBufferedBytes] === 0) {
    return "";
  }

  const ret = bufferToString(
    this.lastChar,
    this.encoding,
    0,
    this[kBufferedBytes],
  );

  this[kBufferedBytes] = 0;
  this[kMissingBytes] = 0;

  return ret;
}

function StringDecoder(encoding) {
  const normalizedEncoding = normalizeEncoding(encoding);
  let enc = ENCODING_UTF8;
  let bufLen = 0;
  switch (normalizedEncoding) {
    case "utf8":
      enc = ENCODING_UTF8;
      bufLen = 4;
      break;
    case "base64":
      enc = ENCODING_BASE64;
      bufLen = 4;
      break;
    case "base64url":
      enc = ENCODING_BASE64URL;
      bufLen = 4;
      break;
    case "utf16le":
      enc = ENCODING_UTF16;
      bufLen = 4;
      break;
    case "hex":
      enc = ENCODING_HEX;
      bufLen = 0;
      break;
    case "latin1":
      enc = ENCODING_LATIN1;
      bufLen = 0;
      break;
    case "ascii":
      enc = ENCODING_ASCII;
      bufLen = 0;
      break;
  }
  this.encoding = normalizedEncoding;
  this.lastChar = Buffer.allocUnsafe(bufLen);
  this.enc = enc;
  this[kBufferedBytes] = 0;
  this[kMissingBytes] = 0;
  this.flush = flush;
  this.decode = decode;
}

StringDecoder.prototype.write = function write(buf) {
  if (typeof buf === "string") {
    return buf;
  }
  const normalizedBuf = normalizeBuffer(buf);
  if (this[kBufferedBytes] === undefined) {
    throw new ERR_INVALID_THIS("StringDecoder");
  }
  return this.decode(normalizedBuf);
};

StringDecoder.prototype.end = function end(buf) {
  let ret = "";
  if (buf !== undefined) {
    ret = this.write(buf);
  }
  if (this[kBufferedBytes] > 0) {
    ret += this.flush();
  }
  return ret;
};

StringDecoder.prototype.text = function text(buf, offset) {
  this[kBufferedBytes] = 0;
  this[kMissingBytes] = 0;
  return this.write(buf.subarray(offset));
};

ObjectDefineProperties(StringDecoder.prototype, {
  lastNeed: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      return this[kMissingBytes];
    },
  },
  lastTotal: {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      return this[kBufferedBytes] + this[kMissingBytes];
    },
  },
});

return {
  StringDecoder,
  default: { StringDecoder },
};
})();
