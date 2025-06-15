// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.

import {
  TransformStream,
  TransformStreamDefaultController,
} from "ext:deno_web/06_streams.js";

import {
  WritableStream,
  WritableStreamDefaultController,
  WritableStreamDefaultWriter,
} from "ext:deno_web/06_streams.js";

import {
  ReadableByteStreamController,
  ReadableStream,
  ReadableStreamBYOBReader,
  ReadableStreamBYOBRequest,
  ReadableStreamDefaultController,
  ReadableStreamDefaultReader,
} from "ext:deno_web/06_streams.js";

import {
  ByteLengthQueuingStrategy,
  CountQueuingStrategy,
} from "ext:deno_web/06_streams.js";
import {
  TextDecoderStream,
  TextEncoderStream,
} from "ext:deno_web/08_text_encoding.js";
import {
  CompressionStream,
  DecompressionStream,
} from "ext:deno_web/14_compression.js";
"use strict";

const _defaultExport1 = {
  ReadableStream,
  ReadableStreamDefaultReader,
  ReadableStreamBYOBReader,
  ReadableStreamBYOBRequest,
  ReadableByteStreamController,
  ReadableStreamDefaultController,
  TransformStream,
  TransformStreamDefaultController,
  WritableStream,
  WritableStreamDefaultWriter,
  WritableStreamDefaultController,
  ByteLengthQueuingStrategy,
  CountQueuingStrategy,
  TextEncoderStream,
  TextDecoderStream,
  CompressionStream,
  DecompressionStream,
};

export default _defaultExport1;
export {
  ByteLengthQueuingStrategy,
  CompressionStream,
  CountQueuingStrategy,
  DecompressionStream,
  ReadableByteStreamController,
  ReadableStream,
  ReadableStreamBYOBReader,
  ReadableStreamBYOBRequest,
  ReadableStreamDefaultController,
  ReadableStreamDefaultReader,
  TextDecoderStream,
  TextEncoderStream,
  TransformStream,
  TransformStreamDefaultController,
  WritableStream,
  WritableStreamDefaultController,
  WritableStreamDefaultWriter,
};
