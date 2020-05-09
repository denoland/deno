// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  CancelAlgorithm,
  dequeueValue,
  isReadableStreamDefaultController,
  Pair,
  PullAlgorithm,
  readableStreamAddReadRequest,
  readableStreamClose,
  readableStreamCreateReadResult,
  readableStreamDefaultControllerCallPullIfNeeded,
  readableStreamDefaultControllerCanCloseOrEnqueue,
  readableStreamDefaultControllerClearAlgorithms,
  readableStreamDefaultControllerClose,
  readableStreamDefaultControllerEnqueue,
  readableStreamDefaultControllerError,
  readableStreamDefaultControllerGetDesiredSize,
  resetQueue,
  SizeAlgorithm,
  setFunctionName,
} from "./internals.ts";
import { ReadableStreamImpl } from "./readable_stream.ts";
import * as sym from "./symbols.ts";
import { customInspect } from "../console.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export class ReadableStreamDefaultControllerImpl<R = any>
  implements ReadableStreamDefaultController<R> {
  [sym.cancelAlgorithm]: CancelAlgorithm;
  [sym.closeRequested]: boolean;
  [sym.controlledReadableStream]: ReadableStreamImpl<R>;
  [sym.pullAgain]: boolean;
  [sym.pullAlgorithm]: PullAlgorithm;
  [sym.pulling]: boolean;
  [sym.queue]: Array<Pair<R>>;
  [sym.queueTotalSize]: number;
  [sym.started]: boolean;
  [sym.strategyHWM]: number;
  [sym.strategySizeAlgorithm]: SizeAlgorithm<R>;

  private constructor() {
    throw new TypeError(
      "ReadableStreamDefaultController's constructor cannot be called."
    );
  }

  get desiredSize(): number | null {
    if (!isReadableStreamDefaultController(this)) {
      throw new TypeError("Invalid ReadableStreamDefaultController.");
    }
    return readableStreamDefaultControllerGetDesiredSize(this);
  }

  close(): void {
    if (!isReadableStreamDefaultController(this)) {
      throw new TypeError("Invalid ReadableStreamDefaultController.");
    }
    if (!readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
      throw new TypeError(
        "ReadableStreamDefaultController cannot close or enqueue."
      );
    }
    readableStreamDefaultControllerClose(this);
  }

  enqueue(chunk: R): void {
    if (!isReadableStreamDefaultController(this)) {
      throw new TypeError("Invalid ReadableStreamDefaultController.");
    }
    if (!readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
      throw new TypeError("ReadableSteamController cannot enqueue.");
    }
    return readableStreamDefaultControllerEnqueue(this, chunk);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  error(error?: any): void {
    if (!isReadableStreamDefaultController(this)) {
      throw new TypeError("Invalid ReadableStreamDefaultController.");
    }
    readableStreamDefaultControllerError(this, error);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [sym.cancelSteps](reason?: any): PromiseLike<void> {
    resetQueue(this);
    const result = this[sym.cancelAlgorithm](reason);
    readableStreamDefaultControllerClearAlgorithms(this);
    return result;
  }

  [sym.pullSteps](): Promise<ReadableStreamReadResult<R>> {
    const stream = this[sym.controlledReadableStream];
    if (this[sym.queue].length) {
      const chunk = dequeueValue<R>(this);
      if (this[sym.closeRequested] && this[sym.queue].length === 0) {
        readableStreamDefaultControllerClearAlgorithms(this);
        readableStreamClose(stream);
      } else {
        readableStreamDefaultControllerCallPullIfNeeded(this);
      }
      return Promise.resolve(
        readableStreamCreateReadResult(
          chunk,
          false,
          stream[sym.reader]![sym.forAuthorCode]
        )
      );
    }
    const pendingPromise = readableStreamAddReadRequest(stream);
    readableStreamDefaultControllerCallPullIfNeeded(this);
    return pendingPromise;
  }

  [customInspect](): string {
    return `${this.constructor.name} { desiredSize: ${String(
      this.desiredSize
    )} }`;
  }
}

setFunctionName(
  ReadableStreamDefaultControllerImpl,
  "ReadableStreamDefaultController"
);
