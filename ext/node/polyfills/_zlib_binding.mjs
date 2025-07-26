// Copyright 2018-2025 the Deno authors. MIT license.

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
export const BROTLI_DECODE = 8;
export const BROTLI_ENCODE = 9;
export const ZSTD_COMPRESS = 10;
export const ZSTD_DECOMPRESS = 11;

export const Z_MIN_WINDOWBITS = 8;
export const Z_MAX_WINDOWBITS = 15;
export const Z_DEFAULT_WINDOWBITS = 15;
export const Z_MIN_CHUNK = 64;
export const Z_MAX_CHUNK = 0x7fffffff;
export const Z_DEFAULT_CHUNK = 16 * 1024;
export const Z_MIN_MEMLEVEL = 1;
export const Z_MAX_MEMLEVEL = 9;
export const Z_DEFAULT_MEMLEVEL = 8;
export const Z_MIN_LEVEL = -1;
export const Z_MAX_LEVEL = 9;
export const Z_DEFAULT_LEVEL = Z_DEFAULT_COMPRESSION;

export const BROTLI_OPERATION_PROCESS = 0;
export const BROTLI_OPERATION_FLUSH = 1;
export const BROTLI_OPERATION_FINISH = 2;
export const BROTLI_OPERATION_EMIT_METADATA = 3;

import {
  BrotliDecoder,
  BrotliEncoder,
  op_zlib_crc32,
  op_zlib_crc32_string,
  Zlib,
} from "ext:core/ops";

function crc32(buf, crc) {
  if (typeof buf === "string") {
    return op_zlib_crc32_string(buf, crc);
  }
  if (buf instanceof DataView) {
    buf = new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
  }
  return op_zlib_crc32(buf, crc);
}

export { BrotliDecoder, BrotliEncoder, crc32, Zlib };

export default { BrotliDecoder, BrotliEncoder, Zlib, crc32 };
