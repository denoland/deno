// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
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
export class Options {
  constructor() {
    notImplemented("Options.prototype.constructor");
  }
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

function brotliMaxCompressedSize(input: number): number {
  if (input == 0) return 2;

  // [window bits / empty metadata] + N * [uncompressed] + [last empty]
  const numLargeBlocks = input >> 24;
  const overhead = 2 + (4 * numLargeBlocks) + 3 + 1;
  const result = input + overhead;

  return result < input ? 0 : result;
}

export function createBrotliCompress() {
  notImplemented("createBrotliCompress");
}
export function createBrotliDecompress() {
  notImplemented("createBrotliDecompress");
}
export function brotliCompress() {
  notImplemented("brotliCompress");
}

export function brotliCompressSync(
  input: Uint8Array,
) {
  const output = new Uint8Array(brotliMaxCompressedSize(input.length));
}

export function brotliDecompress() {
  notImplemented("brotliDecompress");
}
export function brotliDecompressSync() {
  notImplemented("brotliDecompressSync");
}

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
};
