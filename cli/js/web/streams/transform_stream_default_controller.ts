// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  FlushAlgorithm,
  isTransformStreamDefaultController,
  readableStreamDefaultControllerGetDesiredSize,
  setFunctionName,
  TransformAlgorithm,
  transformStreamDefaultControllerEnqueue,
  transformStreamDefaultControllerError,
  transformStreamDefaultControllerTerminate,
} from "./internals.ts";
import { ReadableStreamDefaultControllerImpl } from "./readable_stream_default_controller.ts";
import * as sym from "./symbols.ts";
import { TransformStreamImpl } from "./transform_stream.ts";
import { customInspect } from "../console.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export class TransformStreamDefaultControllerImpl<I = any, O = any>
  implements TransformStreamDefaultController<O> {
  [sym.controlledTransformStream]: TransformStreamImpl<I, O>;
  [sym.flushAlgorithm]: FlushAlgorithm;
  [sym.transformAlgorithm]: TransformAlgorithm<I>;

  private constructor() {
    throw new TypeError(
      "TransformStreamDefaultController's constructor cannot be called."
    );
  }

  get desiredSize(): number | null {
    if (!isTransformStreamDefaultController(this)) {
      throw new TypeError("Invalid TransformStreamDefaultController.");
    }
    const readableController = this[sym.controlledTransformStream][
      sym.readable
    ][sym.readableStreamController];
    return readableStreamDefaultControllerGetDesiredSize(
      readableController as ReadableStreamDefaultControllerImpl<O>
    );
  }

  enqueue(chunk: O): void {
    if (!isTransformStreamDefaultController(this)) {
      throw new TypeError("Invalid TransformStreamDefaultController.");
    }
    transformStreamDefaultControllerEnqueue(this, chunk);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  error(reason?: any): void {
    if (!isTransformStreamDefaultController(this)) {
      throw new TypeError("Invalid TransformStreamDefaultController.");
    }
    transformStreamDefaultControllerError(this, reason);
  }

  terminate(): void {
    if (!isTransformStreamDefaultController(this)) {
      throw new TypeError("Invalid TransformStreamDefaultController.");
    }
    transformStreamDefaultControllerTerminate(this);
  }

  [customInspect](): string {
    return `${this.constructor.name} { desiredSize: ${String(
      this.desiredSize
    )} }`;
  }
}

setFunctionName(
  TransformStreamDefaultControllerImpl,
  "TransformStreamDefaultController"
);
