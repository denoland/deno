// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_web.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { TransformStream } = window.__bootstrap.streams;

  webidl.converters.compressionFormat = webidl.createEnumConverter(
    "compressionFormat",
    [
      "deflate",
      "gzip",
    ],
  );

  const FLUSH_COMPRESS_NONE = 0;
  const FLUSH_COMPRESS_SYNC = 1;
  const FLUSH_COMPRESS_PARTIAL = 2;
  const FLUSH_COMPRESS_FULL = 3;
  const FLUSH_COMPRESS_FINISH = 4;

  const FLUSH_DECOMPRESS_NONE = 0;
  const FLUSH_DECOMPRESS_SYNC = 1;
  const FLUSH_DECOMPRESS_FINISH = 2;

  const STATUS_OK = 0;
  const STATUS_BUF_ERROR = 1;
  const STATUS_STREAM_END = 2;

  function compressTotalInOut(rid) {
    return core.opSync("op_compression_compress_total_in_out", rid);
  }

  function decompressTotalInOut(rid) {
    return core.opSync("op_compression_decompress_total_in_out", rid);
  }

  class CompressionStream extends TransformStream {
    constructor(format) {
      const prefix = "Failed to construct 'CompressionStream'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      format = webidl.converters.compressionFormat(format, {
        prefix,
        context: "Argument 1",
      });

      const rid = core.opSync("op_compression_new", format, false);

      super({
        async transform(chunk, controller) {
          const output = core.opSync(
            "op_compression_write",
            rid,
            chunk,
          );
          maybeEnqueue(controller, output);
        },
        async flush(controller) {
          const output = core.opSync("op_compression_finish", rid);
          maybeEnqueue(controller, output);
        },
      });
    }
  }

  webidl.configurePrototype(CompressionStream);

  class DecompressionStream extends TransformStream {
    constructor(format) {
      const prefix = "Failed to construct 'DecompressionStream'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      format = webidl.converters.compressionFormat(format, {
        prefix,
        context: "Argument 1",
      });

      const rid = core.opSync("op_compression_new", format, true);

      super({
        transform(chunk, controller) {
          const output = core.opSync(
            "op_compression_write",
            rid,
            chunk,
          );
          maybeEnqueue(controller, output);
        },
        flush(controller) {
          const output = core.opSync("op_compression_finish", rid);
          maybeEnqueue(controller, output);
        },
      });
    }
  }

  function maybeEnqueue(controller, output) {
    if (output && output.byteLength > 0) {
      controller.enqueue(output);
    }
  }

  webidl.configurePrototype(DecompressionStream);

  window.__bootstrap.compression = {
    CompressionStream,
    DecompressionStream,
  };
})(globalThis);
