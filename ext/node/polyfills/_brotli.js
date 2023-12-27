// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { zlib as constants } from "ext:deno_node/internal_binding/constants.ts";
import { TextEncoder } from "ext:deno_web/08_text_encoding.js";
import { Transform } from "node:stream";
import { Buffer } from "node:buffer";
const { core } = globalThis.__bootstrap;
const { ops } = core;
const {
  op_brotli_compress_async,
} = core.ensureFastOps();

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

export function createBrotliDecompress(options) {
  return new BrotliDecompress(options);
}

export class BrotliDecompress extends Transform {
  #context;

  // TODO(littledivy): use `options` argument
  constructor(_options = {}) {
    super({
      // TODO(littledivy): use `encoding` argument
      transform(chunk, _encoding, callback) {
        const input = toU8(chunk);
        const output = new Uint8Array(1024);
        const avail = ops.op_brotli_decompress_stream(context, input, output);
        this.push(output.slice(0, avail));
        callback();
      },
      flush(callback) {
        core.close(context);
        callback();
      },
    });

    this.#context = ops.op_create_brotli_decompress();
    const context = this.#context;
  }
}

export class BrotliCompress extends Transform {
  #context;

  constructor(options = {}) {
    super({
      // TODO(littledivy): use `encoding` argument
      transform(chunk, _encoding, callback) {
        const input = toU8(chunk);
        const output = new Uint8Array(brotliMaxCompressedSize(input.length));
        const avail = ops.op_brotli_compress_stream(context, input, output);
        this.push(output.slice(0, avail));
        callback();
      },
      flush(callback) {
        const output = new Uint8Array(1024);
        const avail = ops.op_brotli_compress_stream_end(context, output);
        this.push(output.slice(0, avail));
        callback();
      },
    });

    const params = Object.values(options?.params ?? {});
    this.#context = ops.op_create_brotli_compress(params);
    const context = this.#context;
  }
}

function oneOffCompressOptions(options) {
  const quality = options?.params?.[constants.BROTLI_PARAM_QUALITY] ??
    constants.BROTLI_DEFAULT_QUALITY;
  const lgwin = options?.params?.[constants.BROTLI_PARAM_LGWIN] ??
    constants.BROTLI_DEFAULT_WINDOW;
  const mode = options?.params?.[constants.BROTLI_PARAM_MODE] ??
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

  if (typeof options === "function") {
    callback = options;
    options = {};
  }

  const { quality, lgwin, mode } = oneOffCompressOptions(options);
  op_brotli_compress_async(buf, quality, lgwin, mode)
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
  return Buffer.from(output.subarray(0, len));
}

export function brotliDecompress(input) {
  const buf = toU8(input);
  return ops.op_brotli_decompress_async(buf)
    .then((result) => callback(null, Buffer.from(result)))
    .catch((err) => callback(err));
}

export function brotliDecompressSync(input) {
  return Buffer.from(ops.op_brotli_decompress(toU8(input)));
}
