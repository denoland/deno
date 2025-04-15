// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

"use strict";

import { primordials } from "ext:core/mod.js";
const {
  ObjectDefineProperty,
  ObjectKeys,
  ReflectApply,
} = primordials;

import * as internalUtil from "ext:deno_node/internal/util.mjs";
const {
  promisify: { custom: customPromisify },
} = internalUtil;

import {
  promiseReturningOperators,
  streamReturningOperators,
} from "ext:deno_node/internal/streams/operators.js";

import compose from "ext:deno_node/internal/streams/compose.js";
import {
  getDefaultHighWaterMark,
  setDefaultHighWaterMark,
} from "ext:deno_node/internal/streams/state.js";
import { pipeline } from "ext:deno_node/internal/streams/pipeline.js";
import { destroyer } from "ext:deno_node/internal/streams/destroy.js";
import { eos } from "ext:deno_node/internal/streams/end-of-stream.js";
import { Buffer } from "ext:deno_node/internal/buffer.mjs";

import * as promises from "node:stream/promises";
import * as utils from "ext:deno_node/internal/streams/utils.js";
import {
  isArrayBufferView,
  isUint8Array,
} from "ext:deno_node/internal/util/types.ts";

import { Stream } from "ext:deno_node/internal/streams/legacy.js";
import Readable from "ext:deno_node/internal/streams/readable.js";
import Writable from "ext:deno_node/internal/streams/writable.js";
import Duplex from "ext:deno_node/internal/streams/duplex.js";
import Transform from "ext:deno_node/internal/streams/transform.js";
import PassThrough from "ext:deno_node/internal/streams/passthrough.js";
import duplexPair from "ext:deno_node/internal/streams/duplexpair.js";
import { addAbortSignal } from "ext:deno_node/internal/streams/add-abort-signal.js";

Stream.isDestroyed = utils.isDestroyed;
Stream.isDisturbed = utils.isDisturbed;
Stream.isErrored = utils.isErrored;
Stream.isReadable = utils.isReadable;
Stream.isWritable = utils.isWritable;

Stream.Readable = Readable;
const streamKeys = ObjectKeys(streamReturningOperators);
for (let i = 0; i < streamKeys.length; i++) {
  const key = streamKeys[i];
  const op = streamReturningOperators[key];
  function fn(...args) {
    if (new.target) {
      throw new ERR_ILLEGAL_CONSTRUCTOR();
    }
    return Stream.Readable.from(ReflectApply(op, this, args));
  }
  ObjectDefineProperty(fn, "name", { __proto__: null, value: op.name });
  ObjectDefineProperty(fn, "length", { __proto__: null, value: op.length });
  ObjectDefineProperty(Stream.Readable.prototype, key, {
    __proto__: null,
    value: fn,
    enumerable: false,
    configurable: true,
    writable: true,
  });
}
const promiseKeys = ObjectKeys(promiseReturningOperators);
for (let i = 0; i < promiseKeys.length; i++) {
  const key = promiseKeys[i];
  const op = promiseReturningOperators[key];
  function fn(...args) {
    if (new.target) {
      throw new ERR_ILLEGAL_CONSTRUCTOR();
    }
    return ReflectApply(op, this, args);
  }
  ObjectDefineProperty(fn, "name", { __proto__: null, value: op.name });
  ObjectDefineProperty(fn, "length", { __proto__: null, value: op.length });
  ObjectDefineProperty(Stream.Readable.prototype, key, {
    __proto__: null,
    value: fn,
    enumerable: false,
    configurable: true,
    writable: true,
  });
}
Stream.Writable = Writable;
Stream.Duplex = Duplex;
Stream.Transform = Transform;
Stream.PassThrough = PassThrough;
Stream.duplexPair = duplexPair;
Stream.pipeline = pipeline;

Stream.addAbortSignal = addAbortSignal;
Stream.finished = eos;
Stream.destroy = destroyer;
Stream.compose = compose;
Stream.setDefaultHighWaterMark = setDefaultHighWaterMark;
Stream.getDefaultHighWaterMark = getDefaultHighWaterMark;

ObjectDefineProperty(Stream, "promises", {
  __proto__: null,
  configurable: true,
  enumerable: true,
  get() {
    return promises;
  },
});

ObjectDefineProperty(pipeline, customPromisify, {
  __proto__: null,
  enumerable: true,
  get() {
    return promises.pipeline;
  },
});

ObjectDefineProperty(eos, customPromisify, {
  __proto__: null,
  enumerable: true,
  get() {
    return promises.finished;
  },
});

// Backwards-compat with node 0.4.x
Stream.Stream = Stream;

Stream._isArrayBufferView = isArrayBufferView;
Stream._isUint8Array = isUint8Array;
Stream._uint8ArrayToBuffer = function _uint8ArrayToBuffer(chunk) {
  // Note: Diverging from Node.js here. Deno doesn't implement
  // FastBuffer so we use regular Buffer.
  return Buffer.from(chunk.buffer, chunk.byteOffset, chunk.byteLength);
};

export {
  addAbortSignal,
  compose,
  destroyer,
  Duplex,
  duplexPair,
  getDefaultHighWaterMark,
  PassThrough,
  pipeline,
  Readable,
  setDefaultHighWaterMark,
  Stream,
  Transform,
  Writable,
};
export const _isArrayBufferView = isArrayBufferView;
export const _isUint8Array = Stream._isUint8Array;
export const _uint8ArrayToBuffer = Stream._uint8ArrayToBuffer;
export const isDisturbed = Stream.isDisturbed;
export const isErrored = Stream.isErrored;
export const finished = eos;
export const isReadable = Stream.isReadable;
export const isWritable = Stream.isWritable;
export const isDestroyed = Stream.isDestroyed;

export default Stream;
