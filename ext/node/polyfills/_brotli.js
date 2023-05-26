// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { notImplemented } from "ext:deno_node/_utils.ts";
import { zlib as constants } from "ext:deno_node/internal_binding/constants.ts";
import { TextEncoder } from "ext:deno_web/08_text_encoding.js";
import { Transform } from "ext:deno_node/stream.ts";

const { core } = globalThis.__bootstrap;
const { ops } = core;

const enc = new TextEncoder();
const toU8 = (input) => {
  if (typeof input === "string") {
    return enc.encode(input);
  }

  return input;
};

export function createBrotliCompress(options) {
  return new BrotliCompress(options);
}

export function createBrotliDecompress() {
  notImplemented("createBrotliDecompress");
}

export class BrotliCompress extends Transform {
  #context;

  constructor(options = {}) {
    super({
      transform(chunk, encoding, callback) {
        const input = toU8(chunk);
        const avail = ops.op_brotli_compress_stream(context, input, output);
        this.push(output.slice(0, avail));
        callback();
      },
      flush(callback) {
        callback();
      },
    });

    const params = Object.values(options?.params ?? {});
    this.#context = ops.op_create_brotli_compress(params);
    const context = this.#context;
  }
}

function oneOffCompressOptions(options) {
  const quality = options?.params[constants.BROTLI_PARAM_QUALITY] ??
    constants.BROTLI_DEFAULT_QUALITY;
  const lgwin = options?.params[constants.BROTLI_PARAM_LGWIN] ??
    constants.BROTLI_DEFAULT_WINDOW;
  const mode = options?.params[constants.BROTLI_PARAM_MODE] ??
    constants.BROTLI_MODE_GENERIC;

  return {
    quality,
    lgwin,
    mode,
  };
}

function brotliMaxCompressedSize(input) {
  if (input == 0) return 2;

  // [window bits / empty metadata] + N * [uncompressed] + [last empty]
  const numLargeBlocks = input >> 24;
  const overhead = 2 + (4 * numLargeBlocks) + 3 + 1;
  const result = input + overhead;

  return result < input ? 0 : result;
}

export function brotliCompress(
  input,
  options,
  callback,
) {
  const buf = toU8(input);
  const { quality, lgwin, mode } = oneOffCompressOptions(options);
  core.opAsync("op_brotli_compress_async", buf, quality, lgwin, mode)
    .then((result) => callback(null, result))
    .catch((err) => callback(err));
}

export function brotliCompressSync(
  input,
  options,
) {
  const buf = toU8(input);
  const output = new Uint8Array(brotliMaxCompressedSize(buf.length));

  const { quality, lgwin, mode } = oneOffCompressOptions(options);
  const len = ops.op_brotli_compress(buf, output, quality, lgwin, mode);
  return output.subarray(0, len);
}

export function brotliDecompress() {
  notImplemented("brotliDecompress");
}
export function brotliDecompressSync() {
  notImplemented("brotliDecompressSync");
}
