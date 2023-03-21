// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2014-2015 Devon Govett <devongovett@gmail.com>
// Forked from https://github.com/browserify/browserify-zlib

// deno-lint-ignore-file

import assert from "ext:deno_node/assert.ts";
import { constants, zlib_deflate, zlib_inflate } from "ext:deno_node/_pako.mjs";
import { nextTick } from "ext:deno_node/_next_tick.ts";

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
