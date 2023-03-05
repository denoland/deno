// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2014-2015 Devon Govett <devongovett@gmail.com>
// Forked from https://github.com/browserify/browserify-zlib

// deno-lint-ignore-file

import assert from "internal:deno_node/assert.ts";
import { constants, zlib_deflate, zlib_inflate, Zstream } from "internal:deno_node/_pako.mjs";
import { nextTick } from "internal:deno_node/_next_tick.ts";

export const Z_NO_FLUSH = constants.Z_NO_FLUSH;
export const Z_PARTIAL_FLUSH = constants.Z_PARTIAL_FLUSH;
export const Z_SYNC_FLUSH = constants.Z_SYNC_FLUSH;
export const Z_FULL_FLUSH = constants.Z_FULL_FLUSH;
export const Z_FINISH = constants.Z_FINISH;
export const Z_BLOCK = constants.Z_BLOCK;
export const Z_TREES = constants.Z_TREES;
export const Z_OK = constants.Z_OK;
export const Z_STREAM_END = constants.Z_STREAM_END;
export const Z_NEED_DICT = constants.Z_NEED_DICT;
export const Z_ERRNO = constants.Z_ERRNO;
export const Z_STREAM_ERROR = constants.Z_STREAM_ERROR;
export const Z_DATA_ERROR = constants.Z_DATA_ERROR;
export const Z_MEM_ERROR = constants.Z_MEM_ERROR;
export const Z_BUF_ERROR = constants.Z_BUF_ERROR;
export const Z_VERSION_ERROR = constants.Z_VERSION_ERROR;
export const Z_NO_COMPRESSION = constants.Z_NO_COMPRESSION;
export const Z_BEST_SPEED = constants.Z_BEST_SPEED;
export const Z_BEST_COMPRESSION = constants.Z_BEST_COMPRESSION;
export const Z_DEFAULT_COMPRESSION = constants.Z_DEFAULT_COMPRESSION;
export const Z_FILTERED = constants.Z_FILTERED;
export const Z_HUFFMAN_ONLY = constants.Z_HUFFMAN_ONLYZ_FILTERED;
export const Z_RLE = constants.Z_RLE;
export const Z_FIXED = constants.Z_FIXED;
export const Z_DEFAULT_STRATEGY = constants.Z_DEFAULT_STRATEGY;
export const Z_BINARY = constants.Z_BINARY;
export const Z_TEXT = constants.Z_TEXT;
export const Z_UNKNOWN = constants.Z_UNKNOWN;
export const Z_DEFLATED = constants.Z_DEFLATED;

// zlib modes
export const NONE = 0;
export const DEFLATE = 1;
export const INFLATE = 2;
export const GZIP = 3;
export const GUNZIP = 4;
export const DEFLATERAW = 5;
export const INFLATERAW = 6;
export const UNZIP = 7;

var GZIP_HEADER_ID1 = 0x1f;
var GZIP_HEADER_ID2 = 0x8b;

/**
 * Emulate Node's zlib C++ layer for use by the JS layer in index.js
 */
function Zlib(mode) {
  if (typeof mode !== "number" || mode < DEFLATE || mode > UNZIP) {
    throw new TypeError("Bad argument");
  }

  this.dictionary = null;
  this.err = 0;
  this.flush = 0;
  this.init_done = false;
  this.level = 0;
  this.memLevel = 0;
  this.mode = mode;
  this.strategy = 0;
  this.windowBits = 0;
  this.write_in_progress = false;
  this.pending_close = false;
  this.gzip_id_bytes_read = 0;
}

Zlib.prototype.close = function () {
  if (this.write_in_progress) {
    this.pending_close = true;
    return;
  }

  this.pending_close = false;

  assert(this.init_done, "close before init");
  assert(this.mode <= UNZIP);

  if (this.mode === DEFLATE || this.mode === GZIP || this.mode === DEFLATERAW) {
    zlib_deflate.deflateEnd(this.strm);
  } else if (
    this.mode === INFLATE || this.mode === GUNZIP || this.mode === INFLATERAW ||
    this.mode === UNZIP
  ) {
    zlib_inflate.inflateEnd(this.strm);
  }

  this.mode = NONE;

  this.dictionary = null;
};

Zlib.prototype.write = function (
  flush,
  input,
  in_off,
  in_len,
  out,
  out_off,
  out_len,
) {
  return this._write(true, flush, input, in_off, in_len, out, out_off, out_len);
};

Zlib.prototype.writeSync = function (
  flush,
  input,
  in_off,
  in_len,
  out,
  out_off,
  out_len,
) {
  return this._write(
    false,
    flush,
    input,
    in_off,
    in_len,
    out,
    out_off,
    out_len,
  );
};

Zlib.prototype._write = function (
  async,
  flush,
  input,
  in_off,
  in_len,
  out,
  out_off,
  out_len,
) {
  assert.equal(arguments.length, 8);

  assert(this.init_done, "write before init");
  assert(this.mode !== NONE, "already finalized");
  assert.equal(false, this.write_in_progress, "write already in progress");
  assert.equal(false, this.pending_close, "close is pending");

  this.write_in_progress = true;

  assert.equal(false, flush === undefined, "must provide flush value");

  this.write_in_progress = true;

  if (
    flush !== Z_NO_FLUSH && flush !== Z_PARTIAL_FLUSH &&
    flush !== Z_SYNC_FLUSH && flush !== Z_FULL_FLUSH && flush !== Z_FINISH &&
    flush !== Z_BLOCK
  ) {
    throw new Error("Invalid flush value");
  }

  if (input == null) {
    input = Buffer.alloc(0);
    in_len = 0;
    in_off = 0;
  }

  this.strm.avail_in = in_len;
  this.strm.input = input;
  this.strm.next_in = in_off;
  this.strm.avail_out = out_len;
  this.strm.output = out;
  this.strm.next_out = out_off;
  this.flush = flush;

  if (!async) {
    // sync version
    this._process();

    if (this._checkError()) {
      return this._afterSync();
    }
    return;
  }

  // async version
  var self = this;
  nextTick(function () {
    self._process();
    self._after();
  });

  return this;
};

Zlib.prototype._afterSync = function () {
  var avail_out = this.strm.avail_out;
  var avail_in = this.strm.avail_in;

  this.write_in_progress = false;

  return [avail_in, avail_out];
};

Zlib.prototype._process = function () {
  var next_expected_header_byte = null;

  // If the avail_out is left at 0, then it means that it ran out
  // of room.  If there was avail_out left over, then it means
  // that all of the input was consumed.
  switch (this.mode) {
    case DEFLATE:
    case GZIP:
    case DEFLATERAW:
      this.err = zlib_deflate.deflate(this.strm, this.flush);
      break;
    case UNZIP:
      if (this.strm.avail_in > 0) {
        next_expected_header_byte = this.strm.next_in;
      }

      switch (this.gzip_id_bytes_read) {
        case 0:
          if (next_expected_header_byte === null) {
            break;
          }

          if (this.strm.input[next_expected_header_byte] === GZIP_HEADER_ID1) {
            this.gzip_id_bytes_read = 1;
            next_expected_header_byte++;

            if (this.strm.avail_in === 1) {
              // The only available byte was already read.
              break;
            }
          } else {
            this.mode = INFLATE;
            break;
          }

        // fallthrough

        case 1:
          if (next_expected_header_byte === null) {
            break;
          }

          if (this.strm.input[next_expected_header_byte] === GZIP_HEADER_ID2) {
            this.gzip_id_bytes_read = 2;
            this.mode = GUNZIP;
          } else {
            // There is no actual difference between INFLATE and INFLATERAW
            // (after initialization).
            this.mode = INFLATE;
          }

          break;
        default:
          throw new Error("invalid number of gzip magic number bytes read");
      }

    // fallthrough

    case INFLATE:
    case GUNZIP:
    case INFLATERAW:
      this.err = zlib_inflate.inflate(this.strm, this.flush);

      // If data was encoded with dictionary
      if (this.err === Z_NEED_DICT && this.dictionary) {
        // Load it
        this.err = zlib_inflate.inflateSetDictionary(
          this.strm,
          this.dictionary,
        );
        if (this.err === Z_OK) {
          // And try to decode again
          this.err = zlib_inflate.inflate(this.strm, this.flush);
        } else if (this.err === Z_DATA_ERROR) {
          // Both inflateSetDictionary() and inflate() return Z_DATA_ERROR.
          // Make it possible for After() to tell a bad dictionary from bad
          // input.
          this.err = Z_NEED_DICT;
        }
      }
      while (
        this.strm.avail_in > 0 && this.mode === GUNZIP &&
        this.err === Z_STREAM_END && this.strm.next_in[0] !== 0x00
      ) {
        // Bytes remain in input buffer. Perhaps this is another compressed
        // member in the same archive, or just trailing garbage.
        // Trailing zero bytes are okay, though, since they are frequently
        // used for padding.

        this.reset();
        this.err = zlib_inflate.inflate(this.strm, this.flush);
      }
      break;
    default:
      throw new Error("Unknown mode " + this.mode);
  }
};

Zlib.prototype._checkError = function () {
  // Acceptable error states depend on the type of zlib stream.
  switch (this.err) {
    case Z_OK:
    case Z_BUF_ERROR:
      if (this.strm.avail_out !== 0 && this.flush === Z_FINISH) {
        this._error("unexpected end of file");
        return false;
      }
      break;
    case Z_STREAM_END:
      // normal statuses, not fatal
      break;
    case Z_NEED_DICT:
      if (this.dictionary == null) {
        this._error("Missing dictionary");
      } else {
        this._error("Bad dictionary");
      }
      return false;
    default:
      // something else.
      this._error("Zlib error");
      return false;
  }

  return true;
};

Zlib.prototype._after = function () {
  if (!this._checkError()) {
    return;
  }

  var avail_out = this.strm.avail_out;
  var avail_in = this.strm.avail_in;

  this.write_in_progress = false;

  // call the write() cb
  this.callback(avail_in, avail_out);

  if (this.pending_close) {
    this.close();
  }
};

Zlib.prototype._error = function (message) {
  if (this.strm.msg) {
    message = this.strm.msg;
  }
  this.onerror(message, this.err);

  // no hope of rescue.
  this.write_in_progress = false;
  if (this.pending_close) {
    this.close();
  }
};

Zlib.prototype.init = function (
  windowBits,
  level,
  memLevel,
  strategy,
  dictionary,
) {
  assert(
    arguments.length === 4 || arguments.length === 5,
    "init(windowBits, level, memLevel, strategy, [dictionary])",
  );

  assert(windowBits >= 8 && windowBits <= 15, "invalid windowBits");
  assert(level >= -1 && level <= 9, "invalid compression level");

  assert(memLevel >= 1 && memLevel <= 9, "invalid memlevel");

  assert(
    strategy === Z_FILTERED || strategy === Z_HUFFMAN_ONLY ||
      strategy === Z_RLE || strategy === Z_FIXED ||
      strategy === Z_DEFAULT_STRATEGY,
    "invalid strategy",
  );

  this._init(level, windowBits, memLevel, strategy, dictionary);
  this._setDictionary();
};

Zlib.prototype.params = function () {
  throw new Error("deflateParams Not supported");
};

Zlib.prototype.reset = function () {
  this._reset();
  this._setDictionary();
};

Zlib.prototype._init = function (
  level,
  windowBits,
  memLevel,
  strategy,
  dictionary,
) {
  this.level = level;
  this.windowBits = windowBits;
  this.memLevel = memLevel;
  this.strategy = strategy;

  this.flush = Z_NO_FLUSH;

  this.err = Z_OK;

  if (this.mode === GZIP || this.mode === GUNZIP) {
    this.windowBits += 16;
  }

  if (this.mode === UNZIP) {
    this.windowBits += 32;
  }

  if (this.mode === DEFLATERAW || this.mode === INFLATERAW) {
    this.windowBits = -1 * this.windowBits;
  }

  this.strm = new Zstream();

  switch (this.mode) {
    case DEFLATE:
    case GZIP:
    case DEFLATERAW:
      this.err = zlib_deflate.deflateInit2(
        this.strm,
        this.level,
        Z_DEFLATED,
        this.windowBits,
        this.memLevel,
        this.strategy,
      );
      break;
    case INFLATE:
    case GUNZIP:
    case INFLATERAW:
    case UNZIP:
      this.err = zlib_inflate.inflateInit2(this.strm, this.windowBits);
      break;
    default:
      throw new Error("Unknown mode " + this.mode);
  }

  if (this.err !== Z_OK) {
    this._error("Init error");
  }

  this.dictionary = dictionary;

  this.write_in_progress = false;
  this.init_done = true;
};

Zlib.prototype._setDictionary = function () {
  if (this.dictionary == null) {
    return;
  }

  this.err = Z_OK;

  switch (this.mode) {
    case DEFLATE:
    case DEFLATERAW:
      this.err = zlib_deflate.deflateSetDictionary(this.strm, this.dictionary);
      break;
    default:
      break;
  }

  if (this.err !== Z_OK) {
    this._error("Failed to set dictionary");
  }
};

Zlib.prototype._reset = function () {
  this.err = Z_OK;

  switch (this.mode) {
    case DEFLATE:
    case DEFLATERAW:
    case GZIP:
      this.err = zlib_deflate.deflateReset(this.strm);
      break;
    case INFLATE:
    case INFLATERAW:
    case GUNZIP:
      this.err = zlib_inflate.inflateReset(this.strm);
      break;
    default:
      break;
  }

  if (this.err !== Z_OK) {
    this._error("Failed to reset stream");
  }
};

export { Zlib };
