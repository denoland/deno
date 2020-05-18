// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  Deferred,
  getDeferred,
  initializeTransformStream,
  invokeOrNoop,
  isTransformStream,
  makeSizeAlgorithmFromSizeFunction,
  setFunctionName,
  setUpTransformStreamDefaultControllerFromTransformer,
  validateAndNormalizeHighWaterMark,
} from "./internals.ts";
import { ReadableStreamImpl } from "./readable_stream.ts";
import * as sym from "./symbols.ts";
import { TransformStreamDefaultControllerImpl } from "./transform_stream_default_controller.ts";
import { WritableStreamImpl } from "./writable_stream.ts";
import { customInspect, inspect } from "../console.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export class TransformStreamImpl<I = any, O = any>
  implements TransformStream<I, O> {
  [sym.backpressure]?: boolean;
  [sym.backpressureChangePromise]?: Deferred<void>;
  [sym.readable]: ReadableStreamImpl<O>;
  [sym.transformStreamController]: TransformStreamDefaultControllerImpl<I, O>;
  [sym.writable]: WritableStreamImpl<I>;

  constructor(
    transformer: Transformer<I, O> = {},
    writableStrategy: QueuingStrategy<I> = {},
    readableStrategy: QueuingStrategy<O> = {}
  ) {
    const writableSizeFunction = writableStrategy.size;
    let writableHighWaterMark = writableStrategy.highWaterMark;
    const readableSizeFunction = readableStrategy.size;
    let readableHighWaterMark = readableStrategy.highWaterMark;
    const writableType = transformer.writableType;
    if (writableType !== undefined) {
      throw new RangeError(
        `Expected transformer writableType to be undefined, received "${String(
          writableType
        )}"`
      );
    }
    const writableSizeAlgorithm = makeSizeAlgorithmFromSizeFunction(
      writableSizeFunction
    );
    if (writableHighWaterMark === undefined) {
      writableHighWaterMark = 1;
    }
    writableHighWaterMark = validateAndNormalizeHighWaterMark(
      writableHighWaterMark
    );
    const readableType = transformer.readableType;
    if (readableType !== undefined) {
      throw new RangeError(
        `Expected transformer readableType to be undefined, received "${String(
          readableType
        )}"`
      );
    }
    const readableSizeAlgorithm = makeSizeAlgorithmFromSizeFunction(
      readableSizeFunction
    );
    if (readableHighWaterMark === undefined) {
      readableHighWaterMark = 1;
    }
    readableHighWaterMark = validateAndNormalizeHighWaterMark(
      readableHighWaterMark
    );
    const startPromise = getDeferred<void>();
    initializeTransformStream(
      this,
      startPromise.promise,
      writableHighWaterMark,
      writableSizeAlgorithm,
      readableHighWaterMark,
      readableSizeAlgorithm
    );
    // the brand check expects this, and the brand check occurs in the following
    // but the property hasn't been defined.
    Object.defineProperty(this, sym.transformStreamController, {
      value: undefined,
      writable: true,
      configurable: true,
    });
    setUpTransformStreamDefaultControllerFromTransformer(this, transformer);
    const startResult: void | PromiseLike<void> = invokeOrNoop(
      transformer,
      "start",
      this[sym.transformStreamController]
    );
    startPromise.resolve(startResult);
  }

  get readable(): ReadableStream<O> {
    if (!isTransformStream(this)) {
      throw new TypeError("Invalid TransformStream.");
    }
    return this[sym.readable];
  }

  get writable(): WritableStream<I> {
    if (!isTransformStream(this)) {
      throw new TypeError("Invalid TransformStream.");
    }
    return this[sym.writable];
  }

  [customInspect](): string {
    return `${this.constructor.name} {\n  readable: ${inspect(
      this.readable
    )}\n  writable: ${inspect(this.writable)}\n}`;
  }
}

setFunctionName(TransformStreamImpl, "TransformStream");
