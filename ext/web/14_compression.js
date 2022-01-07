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
      const rid = core.opSync("op_compression_compressor_create", format);

      /** @type {Promise<void>} */
      let readPromise;

      super({
        start(controller) {
          readPromise = (async () => {
            while (true) {
              const chunk = new Uint8Array(65536);
              const read = await core.read(rid, chunk);
              if (read === null || read === 0) {
                break;
              } else {
                controller.enqueue(chunk.subarray(0, read));
              }
            }
          })();
        },
        async transform(chunk) {
          const data = webidl.converters.BufferSource(chunk);
          let nwritten = 0;
          while (nwritten < data.byteLength) {
            nwritten += await core.write(rid, data.subarray(nwritten));
          }
        },
        async flush() {
          await core.shutdown(rid);
          await readPromise;
          core.close(rid);
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
      const rid = core.opSync("op_compression_decompressor_create", format);

      /** @type {Promise<void>} */
      let readPromise;

      super({
        start(controller) {
          readPromise = (async () => {
            while (true) {
              const chunk = new Uint8Array(65536);
              const read = await core.read(rid, chunk);
              if (read === null || read === 0) {
                break;
              } else {
                controller.enqueue(chunk.subarray(0, read));
              }
            }
          })();
        },
        async transform(chunk) {
          const data = webidl.converters.BufferSource(chunk);
          let nwritten = 0;
          while (nwritten < data.byteLength) {
            nwritten += await core.write(rid, data.subarray(nwritten));
          }
        },
        async flush() {
          await core.shutdown(rid);
          await readPromise;
          core.close(rid);
        },
      });
    }
  }

  webidl.configurePrototype(DecompressionStream);

  window.__bootstrap.compression = {
    CompressionStream,
    DecompressionStream,
  };
})(globalThis);
