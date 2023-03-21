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

const { ops } = globalThis.__bootstrap.core;

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
    ops.op_zlib_write(
      this.#handle,
      flush,
      input,
      in_off,
      out,
      out_off,
      writeResult,
    );

    return [writeResult[0], writeResult[1]];
  }


  write(
    flush,
    input,
    in_off,
    in_len,
    out,
    out_off,
    out_len,
  ) {}

  init(
    windowBits,
    level,
    memLevel,
    strategy,
    dictionary, 
  ) {
    ops.op_zlib_init(
      this.#handle,
      level,
      windowBits,
      memLevel,
      strategy,
      dictionary,
    );
  }

  params() {
    throw new Error("deflateParams Not supported");
  }

  reset() {
    ops.op_zlib_reset(this.#handle);
  }
}

export { Zlib };
