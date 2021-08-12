// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_web.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

  webidl.converters.compressionFormat = webidl.createEnumConverter(
    "compressionFormat",
    [
      "deflate",
      "gzip",
    ],
  ); // TODO: not per spec, but close enough

  class CompressionStream extends TransformStream {
    constructor(format) {
      const prefix = "Failed to construct 'CompressionStream'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      format = webidl.converters.compressionFormat(format, {
        prefix,
        context: "Argument 1",
      });
      const rid = core.opSync("op_compression_create_compressor", format);

      super({
        transform: (chunk, controller) => {
          const data = webidl.converters.BufferSource(chunk); // TODO: info?
          const buffer = core.opSync("op_compression_compress", {
            format,
            rid,
            data,
          }); // TODO: compress
          controller.enqueue(buffer);
        },
        flush: (controller) => {
          const buffer = core.opSync(
            "op_compression_compress_finalize",
            format,
            rid,
          );
          if (buffer.byteLength !== 0) {
            controller.enqueue(buffer);
          }
        },
      });
    }
  }

  class DecompressionStream extends TransformStream {
    constructor(format) {
      const prefix = "Failed to construct 'DecompressionStream'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      format = webidl.converters.compressionFormat(format, {
        prefix,
        context: "Argument 1",
      });
      const rid = core.opSync("op_compression_create_decompressor", format);

      super({
        transform: (chunk, controller) => {
          const data = webidl.converters.BufferSource(chunk); // TODO: info?
          const buffer = core.opSync("op_compression_decompress", {
            format,
            rid,
            data,
          }); // TODO: compress
          controller.enqueue(buffer);
        },
        flush: (controller) => {
          const buffer = core.opSync(
            "op_compression_decompress_finalize",
            format,
            rid,
          );
          if (buffer.byteLength !== 0) {
            controller.enqueue(buffer);
          }
        },
      });
    }
  }

  window.__bootstrap.compression = {
    CompressionStream,
    DecompressionStream,
  };
})(globalThis);
