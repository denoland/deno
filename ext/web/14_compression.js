// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_web.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

  webidl.converters.compressionFormat = webidl.createEnumConverter("compressionFormat", [
    "deflate",
    "gzip",
  ]); // TODO: not per spec, but close enough

  class CompressionStream extends TransformStream {
    constructor(format) {
      const prefix = "Failed to construct 'CompressionStream'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      format = webidl.converters.compressionFormat(format, {
        prefix,
        context: "Argument 1",
      });

      super({
        transform: (chunk, controller) => {
          chunk = webidl.converters.BufferSource(chunk); // TODO: info?
          const buffer = core.opSync("op_compression"); // TODO: compress
          controller.enqueue(buffer);
        },
        flush: (controller) => {
          // TODO: https://wicg.github.io/compression/#compress-flush-and-enqueue
        }
      });
    }
  }

  window.__bootstrap.compression = {
    CompressionStream,
  };
})(globalThis);
