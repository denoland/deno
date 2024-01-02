// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

// https://github.com/nodeca/pako/blob/master/lib/zlib/constants.js
export const Z_NO_FLUSH = 0;
export const Z_PARTIAL_FLUSH = 1;
export const Z_SYNC_FLUSH = 2;
export const Z_FULL_FLUSH = 3;
export const Z_FINISH = 4;
export const Z_BLOCK = 5;
export const Z_TREES = 6;
export const Z_OK = 0;
export const Z_STREAM_END = 1;
export const Z_NEED_DICT = 2;
export const Z_ERRNO = -1;
export const Z_STREAM_ERROR = -2;
export const Z_DATA_ERROR = -3;
export const Z_MEM_ERROR = -4;
export const Z_BUF_ERROR = -5;
export const Z_VERSION_ERROR = -6;
export const Z_NO_COMPRESSION = 0;
export const Z_BEST_SPEED = 1;
export const Z_BEST_COMPRESSION = 9;
export const Z_DEFAULT_COMPRESSION = -1;
export const Z_FILTERED = 1;
export const Z_HUFFMAN_ONLY = 2;
export const Z_RLE = 3;
export const Z_FIXED = 4;
export const Z_DEFAULT_STRATEGY = 0;
export const Z_BINARY = 0;
export const Z_TEXT = 1;
export const Z_UNKNOWN = 2;
export const Z_DEFLATED = 8;

// zlib modes
export const NONE = 0;
export const DEFLATE = 1;
export const INFLATE = 2;
export const GZIP = 3;
export const GUNZIP = 4;
export const DEFLATERAW = 5;
export const INFLATERAW = 6;
export const UNZIP = 7;

const { core } = globalThis.__bootstrap;
const { ops } = core;
const {
  op_zlib_write_async,
} = core.ensureFastOps();

const writeResult = new Uint32Array(2);

class Zlib {
  #handle;

  constructor(mode) {
    this.#handle = ops.op_zlib_new(mode);
  }

  close() {
    ops.op_zlib_close(this.#handle);
  }

  writeSync(
    flush,
    input,
    in_off,
    in_len,
    out,
    out_off,
    out_len,
  ) {
    const err = ops.op_zlib_write(
      this.#handle,
      flush,
      input,
      in_off,
      in_len,
      out,
      out_off,
      out_len,
      writeResult,
    );

    if (this.#checkError(err)) {
      return [writeResult[1], writeResult[0]];
    }
    return;
  }

  #checkError(err) {
    // Acceptable error states depend on the type of zlib stream.
    switch (err) {
      case Z_BUF_ERROR:
        this.#error("unexpected end of file", err);
        return false;
      case Z_OK:
      case Z_STREAM_END:
        // normal statuses, not fatal
        break;
      case Z_NEED_DICT:
        this.#error("Bad dictionary", err);
        return false;
      default:
        // something else.
        this.#error("Zlib error", err);
        return false;
    }

    return true;
  }

  write(
    flush,
    input,
    in_off,
    in_len,
    out,
    out_off,
    out_len,
  ) {
    op_zlib_write_async(
      this.#handle,
      flush ?? Z_NO_FLUSH,
      input,
      in_off,
      in_len,
      out,
      out_off,
      out_len,
    ).then(([err, availOut, availIn]) => {
      if (this.#checkError(err)) {
        this.callback(availIn, availOut);
      }
    });

    return this;
  }

  init(
    windowBits,
    level,
    memLevel,
    strategy,
    dictionary,
  ) {
    const err = ops.op_zlib_init(
      this.#handle,
      level,
      windowBits,
      memLevel,
      strategy,
      dictionary ?? new Uint8Array(0),
    );

    if (err != Z_OK) {
      this.#error("Failed to initialize zlib", err);
    }
  }

  params() {
    throw new Error("deflateParams Not supported");
  }

  reset() {
    const err = ops.op_zlib_reset(this.#handle);
    if (err != Z_OK) {
      this.#error("Failed to reset stream", err);
    }
  }

  #error(message, err) {
    this.onerror(message, err);
    ops.op_zlib_close_if_pending(this.#handle);
  }
}

export { Zlib };
