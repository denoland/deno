// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { notImplemented } from "internal:deno_node/polyfills/_utils.ts";
import { zlib as constants } from "internal:deno_node/polyfills/internal_binding/constants.ts";
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
} from "internal:deno_node/polyfills/_zlib.mjs";
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
export function createBrotliCompress() {
  notImplemented("createBrotliCompress");
}
export function createBrotliDecompress() {
  notImplemented("createBrotliDecompress");
}
export function brotliCompress() {
  notImplemented("brotliCompress");
}
export function brotliCompressSync() {
  notImplemented("brotliCompressSync");
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

interface ZlibOptions {
  flush?: number;
  finishFlush?: number;
  chunkSize?: number;
  windowBits?: number;
  level?: number;
  memLevel?: number;
  strategy?: number;
  dictionary?: Buffer | ArrayBuffer | ArrayBufferView;
  info?: boolean;
  maxOutputLength?: number;
}
const { ops } = globalThis.__bootstrap.core;

function deflateSync(
  buffer: Buffer | ArrayBuffer | ArrayBufferView,
  options?: ZlibOptions,
) {
  return ops.op_zlib_deflate_sync(
    buffer,
  );
}

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
