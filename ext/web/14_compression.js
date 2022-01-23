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
