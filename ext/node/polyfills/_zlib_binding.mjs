// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file

// https://github.com/nodeca/pako/blob/master/lib/zlib/constants.js
// deno-fmt-ignore-file
(function () {
  const { core } = globalThis.__bootstrap;
  const Z_NO_FLUSH = 0;
  const Z_PARTIAL_FLUSH = 1;
  const Z_SYNC_FLUSH = 2;
  const Z_FULL_FLUSH = 3;
  const Z_FINISH = 4;
  const Z_BLOCK = 5;
  const Z_TREES = 6;
  const Z_OK = 0;
  const Z_STREAM_END = 1;
  const Z_NEED_DICT = 2;
  const Z_ERRNO = -1;
  const Z_STREAM_ERROR = -2;
  const Z_DATA_ERROR = -3;
  const Z_MEM_ERROR = -4;
  const Z_BUF_ERROR = -5;
  const Z_VERSION_ERROR = -6;
  const Z_NO_COMPRESSION = 0;
  const Z_BEST_SPEED = 1;
  const Z_BEST_COMPRESSION = 9;
  const Z_DEFAULT_COMPRESSION = -1;
  const Z_FILTERED = 1;
  const Z_HUFFMAN_ONLY = 2;
  const Z_RLE = 3;
  const Z_FIXED = 4;
  const Z_DEFAULT_STRATEGY = 0;
  const Z_BINARY = 0;
  const Z_TEXT = 1;
  const Z_UNKNOWN = 2;
  const Z_DEFLATED = 8;

  // zlib modes
  const NONE = 0;
  const DEFLATE = 1;
  const INFLATE = 2;
  const GZIP = 3;
  const GUNZIP = 4;
  const DEFLATERAW = 5;
  const INFLATERAW = 6;
  const UNZIP = 7;
  const BROTLI_DECODE = 8;
  const BROTLI_ENCODE = 9;
  const ZSTD_COMPRESS = 10;
  const ZSTD_DECOMPRESS = 11;

  const Z_MIN_WINDOWBITS = 8;
  const Z_MAX_WINDOWBITS = 15;
  const Z_DEFAULT_WINDOWBITS = 15;
  const Z_MIN_CHUNK = 64;
  const Z_MAX_CHUNK = 0x7fffffff;
  const Z_DEFAULT_CHUNK = 16 * 1024;
  const Z_MIN_MEMLEVEL = 1;
  const Z_MAX_MEMLEVEL = 9;
  const Z_DEFAULT_MEMLEVEL = 8;
  const Z_MIN_LEVEL = -1;
  const Z_MAX_LEVEL = 9;
  const Z_DEFAULT_LEVEL = Z_DEFAULT_COMPRESSION;

  const BROTLI_OPERATION_PROCESS = 0;
  const BROTLI_OPERATION_FLUSH = 1;
  const BROTLI_OPERATION_FINISH = 2;
  const BROTLI_OPERATION_EMIT_METADATA = 3;

  const { BrotliDecoder, BrotliEncoder, op_zlib_crc32, op_zlib_crc32_string, Zlib, ZstdCompress, ZstdDecompress } = core.ops;

  function crc32(buf, crc) {
    if (typeof buf === "string") {
      return op_zlib_crc32_string(buf, crc);
    }
    if (buf instanceof DataView) {
      buf = new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
    }
    return op_zlib_crc32(buf, crc);
  }


  const __default_export__ = {
    BrotliDecoder,
    BrotliEncoder,
    Zlib,
    ZstdCompress,
    ZstdDecompress,
    crc32,
  };

  return {
    BrotliDecoder,
    BrotliEncoder,
    crc32,
    Zlib,
    ZstdCompress,
    ZstdDecompress,
    Z_NO_FLUSH,
    Z_PARTIAL_FLUSH,
    Z_SYNC_FLUSH,
    Z_FULL_FLUSH,
    Z_FINISH,
    Z_BLOCK,
    Z_TREES,
    Z_OK,
    Z_STREAM_END,
    Z_NEED_DICT,
    Z_ERRNO,
    Z_STREAM_ERROR,
    Z_DATA_ERROR,
    Z_MEM_ERROR,
    Z_BUF_ERROR,
    Z_VERSION_ERROR,
    Z_NO_COMPRESSION,
    Z_BEST_SPEED,
    Z_BEST_COMPRESSION,
    Z_DEFAULT_COMPRESSION,
    Z_FILTERED,
    Z_HUFFMAN_ONLY,
    Z_RLE,
    Z_FIXED,
    Z_DEFAULT_STRATEGY,
    Z_BINARY,
    Z_TEXT,
    Z_UNKNOWN,
    Z_DEFLATED,
    NONE,
    DEFLATE,
    INFLATE,
    GZIP,
    GUNZIP,
    DEFLATERAW,
    INFLATERAW,
    UNZIP,
    BROTLI_DECODE,
    BROTLI_ENCODE,
    ZSTD_COMPRESS,
    ZSTD_DECOMPRESS,
    Z_MIN_WINDOWBITS,
    Z_MAX_WINDOWBITS,
    Z_DEFAULT_WINDOWBITS,
    Z_MIN_CHUNK,
    Z_MAX_CHUNK,
    Z_DEFAULT_CHUNK,
    Z_MIN_MEMLEVEL,
    Z_MAX_MEMLEVEL,
    Z_DEFAULT_MEMLEVEL,
    Z_MIN_LEVEL,
    Z_MAX_LEVEL,
    Z_DEFAULT_LEVEL,
    BROTLI_OPERATION_PROCESS,
    BROTLI_OPERATION_FLUSH,
    BROTLI_OPERATION_FINISH,
    BROTLI_OPERATION_EMIT_METADATA,
    default: __default_export__,
  };
})()
