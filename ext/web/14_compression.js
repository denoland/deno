// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_web.d.ts" />

const core = globalThis.Deno.core;
const ops = core.ops;
const primordials = globalThis.__bootstrap.primordials;
const {
  TypedArrayPrototypeGetByteLength,
} = primordials;
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { TransformStream } from "ext:deno_web/06_streams.js";

webidl.converters.CompressionFormat = webidl.createEnumConverter(
  "CompressionFormat",
  [
    "deflate",
    "deflate-raw",
    "gzip",
  ],
);

class CompressionStream {
  #transform;

  constructor(format) {
    const prefix = "Failed to construct 'CompressionStream'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    format = webidl.converters.CompressionFormat(format, {
      prefix,
      context: "Argument 1",
    });

    const rid = ops.op_compression_new(format, false);

    this.#transform = new TransformStream({
      transform(chunk, controller) {
        chunk = webidl.converters.BufferSource(chunk, {
          prefix,
          context: "chunk",
        });
        const output = ops.op_compression_write(
          rid,
          chunk,
        );
        maybeEnqueue(controller, output);
      },
      flush(controller) {
        const output = ops.op_compression_finish(rid);
        maybeEnqueue(controller, output);
      },
    });

    this[webidl.brand] = webidl.brand;
  }

  get readable() {
    webidl.assertBranded(this, CompressionStreamPrototype);
    return this.#transform.readable;
  }

  get writable() {
    webidl.assertBranded(this, CompressionStreamPrototype);
    return this.#transform.writable;
  }
}

webidl.configurePrototype(CompressionStream);
const CompressionStreamPrototype = CompressionStream.prototype;

class DecompressionStream {
  #transform;

  constructor(format) {
    const prefix = "Failed to construct 'DecompressionStream'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    format = webidl.converters.CompressionFormat(format, {
      prefix,
      context: "Argument 1",
    });

    const rid = ops.op_compression_new(format, true);

    this.#transform = new TransformStream({
      transform(chunk, controller) {
        chunk = webidl.converters.BufferSource(chunk, {
          prefix,
          context: "chunk",
        });
        const output = ops.op_compression_write(
          rid,
          chunk,
        );
        maybeEnqueue(controller, output);
      },
      flush(controller) {
        const output = ops.op_compression_finish(rid);
        maybeEnqueue(controller, output);
      },
    });

    this[webidl.brand] = webidl.brand;
  }

  get readable() {
    webidl.assertBranded(this, DecompressionStreamPrototype);
    return this.#transform.readable;
  }

  get writable() {
    webidl.assertBranded(this, DecompressionStreamPrototype);
    return this.#transform.writable;
  }
}

function maybeEnqueue(controller, output) {
  if (output && TypedArrayPrototypeGetByteLength(output) > 0) {
    controller.enqueue(output);
  }
}

webidl.configurePrototype(DecompressionStream);
const DecompressionStreamPrototype = DecompressionStream.prototype;

export { CompressionStream, DecompressionStream };
