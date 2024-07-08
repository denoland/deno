// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_web.d.ts" />

import { primordials } from "ext:core/mod.js";
import {
  op_compression_finish,
  op_compression_new,
  op_compression_write,
} from "ext:core/ops";
const {
  SymbolFor,
  ObjectPrototypeIsPrototypeOf,
  TypedArrayPrototypeGetByteLength,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import { TransformStream } from "./06_streams.js";

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
    format = webidl.converters.CompressionFormat(format, prefix, "Argument 1");

    const rid = op_compression_new(format, false);

    this.#transform = new TransformStream({
      transform(chunk, controller) {
        chunk = webidl.converters.BufferSource(chunk, prefix, "chunk");
        const output = op_compression_write(
          rid,
          chunk,
        );
        maybeEnqueue(controller, output);
      },
      flush(controller) {
        const output = op_compression_finish(rid, true);
        maybeEnqueue(controller, output);
      },
      cancel: (_reason) => {
        op_compression_finish(rid, false);
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

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          CompressionStreamPrototype,
          this,
        ),
        keys: [
          "readable",
          "writable",
        ],
      }),
      inspectOptions,
    );
  }
}

webidl.configureInterface(CompressionStream);
const CompressionStreamPrototype = CompressionStream.prototype;

class DecompressionStream {
  #transform;

  constructor(format) {
    const prefix = "Failed to construct 'DecompressionStream'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    format = webidl.converters.CompressionFormat(format, prefix, "Argument 1");

    const rid = op_compression_new(format, true);

    this.#transform = new TransformStream({
      transform(chunk, controller) {
        chunk = webidl.converters.BufferSource(chunk, prefix, "chunk");
        const output = op_compression_write(
          rid,
          chunk,
        );
        maybeEnqueue(controller, output);
      },
      flush(controller) {
        const output = op_compression_finish(rid, true);
        maybeEnqueue(controller, output);
      },
      cancel: (_reason) => {
        op_compression_finish(rid, false);
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

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(
          DecompressionStreamPrototype,
          this,
        ),
        keys: [
          "readable",
          "writable",
        ],
      }),
      inspectOptions,
    );
  }
}

function maybeEnqueue(controller, output) {
  if (output && TypedArrayPrototypeGetByteLength(output) > 0) {
    controller.enqueue(output);
  }
}

webidl.configureInterface(DecompressionStream);
const DecompressionStreamPrototype = DecompressionStream.prototype;

export { CompressionStream, DecompressionStream };
