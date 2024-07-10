// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { notImplemented } from "ext:deno_node/_utils.ts";
import { zlib as constants } from "ext:deno_node/internal_binding/constants.ts";
import {
  codes,
  createDeflate,
  createDeflateRaw,
  createGunzip,
  createGzip,
  createInflate,
  createInflateRaw,
  createUnzip,
  Deflate,
  deflate,
  DeflateRaw,
  deflateRaw,
  deflateRawSync,
  deflateSync,
  Gunzip,
  gunzip,
  gunzipSync,
  Gzip,
  gzip,
  gzipSync,
  Inflate,
  inflate,
  InflateRaw,
  inflateRaw,
  inflateRawSync,
  inflateSync,
  Unzip,
  unzip,
  unzipSync,
} from "ext:deno_node/_zlib.mjs";
import {
  brotliCompress,
  brotliCompressSync,
  brotliDecompress,
  brotliDecompressSync,
  createBrotliCompress,
  createBrotliDecompress,
} from "ext:deno_node/_brotli.js";

export class Options {
  constructor() {
    notImplemented("Options.prototype.constructor");
  }
}

interface IBrotliOptions {
  flush?: number;
  finishFlush?: number;
  chunkSize?: number;
  params?: Record<number, number>;
  maxOutputLength?: number;
}

export class BrotliOptions {
  constructor() {
    notImplemented("BrotliOptions.prototype.constructor");
  }
}
export class BrotliCompress {
  constructor() {
    notImplemented("BrotliCompress.prototype.constructor");
  }
}
export class BrotliDecompress {
  constructor() {
    notImplemented("BrotliDecompress.prototype.constructor");
  }
}
export class ZlibBase {
  constructor() {
    notImplemented("ZlibBase.prototype.constructor");
  }
}
export { constants };

export default {
  Options,
  BrotliOptions,
  BrotliCompress,
  BrotliDecompress,
  Deflate,
  DeflateRaw,
  Gunzip,
  Gzip,
  Inflate,
  InflateRaw,
  Unzip,
  ZlibBase,
  constants,
  codes,
  createBrotliCompress,
  createBrotliDecompress,
  createDeflate,
  createDeflateRaw,
  createGunzip,
  createGzip,
  createInflate,
  createInflateRaw,
  createUnzip,
  brotliCompress,
  brotliCompressSync,
  brotliDecompress,
  brotliDecompressSync,
  deflate,
  deflateSync,
  deflateRaw,
  deflateRawSync,
  gunzip,
  gunzipSync,
  gzip,
  gzipSync,
  inflate,
  inflateSync,
  inflateRaw,
  inflateRawSync,
  unzip,
  unzipSync,
};

export {
  brotliCompress,
  brotliCompressSync,
  brotliDecompress,
  brotliDecompressSync,
  codes,
  createBrotliCompress,
  createBrotliDecompress,
  createDeflate,
  createDeflateRaw,
  createGunzip,
  createGzip,
  createInflate,
  createInflateRaw,
  createUnzip,
  Deflate,
  deflate,
  DeflateRaw,
  deflateRaw,
  deflateRawSync,
  deflateSync,
  Gunzip,
  gunzip,
  gunzipSync,
  Gzip,
  gzip,
  gzipSync,
  Inflate,
  inflate,
  InflateRaw,
  inflateRaw,
  inflateRawSync,
  inflateSync,
  Unzip,
  unzip,
  unzipSync,
};
