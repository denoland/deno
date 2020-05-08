// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  BufferQueueItem,
  CancelAlgorithm,
  isDetachedBuffer,
  isReadableByteStreamController,
  PullAlgorithm,
  resetQueue,
  readableByteStreamControllerCallPullIfNeeded,
  readableByteStreamControllerClearAlgorithms,
  readableByteStreamControllerClose,
  readableByteStreamControllerEnqueue,
  readableByteStreamControllerError,
  readableByteStreamControllerGetDesiredSize,
  readableByteStreamControllerHandleQueueDrain,
  readableStreamAddReadRequest,
  readableStreamHasDefaultReader,
  readableStreamGetNumReadRequests,
  readableStreamCreateReadResult,
  setFunctionName,
} from "./internals.ts";
import { ReadableStreamImpl } from "./readable_stream.ts";
import * as sym from "./symbols.ts";
import { assert } from "../../util.ts";
import { customInspect } from "../console.ts";

export class ReadableByteStreamControllerImpl
  implements ReadableByteStreamController {
  [sym.autoAllocateChunkSize]: number | undefined;
  [sym.byobRequest]: undefined;
  [sym.cancelAlgorithm]: CancelAlgorithm;
  [sym.closeRequested]: boolean;
  [sym.controlledReadableByteStream]: ReadableStreamImpl<Uint8Array>;
  [sym.pullAgain]: boolean;
  [sym.pullAlgorithm]: PullAlgorithm;
  [sym.pulling]: boolean;
  [sym.queue]: BufferQueueItem[];
  [sym.queueTotalSize]: number;
  [sym.started]: boolean;
  [sym.strategyHWM]: number;

  private constructor() {
    throw new TypeError(
      "ReadableByteStreamController's constructor cannot be called."
    );
  }

  get byobRequest(): undefined {
    return undefined;
  }

  get desiredSize(): number | null {
    if (!isReadableByteStreamController(this)) {
      throw new TypeError("Invalid ReadableByteStreamController.");
    }
    return readableByteStreamControllerGetDesiredSize(this);
  }

  close(): void {
    if (!isReadableByteStreamController(this)) {
      throw new TypeError("Invalid ReadableByteStreamController.");
    }
    if (this[sym.closeRequested]) {
      throw new TypeError("Closed already requested.");
    }
    if (this[sym.controlledReadableByteStream][sym.state] !== "readable") {
      throw new TypeError(
        "ReadableByteStreamController's stream is not in a readable state."
      );
    }
    readableByteStreamControllerClose(this);
  }

  enqueue(chunk: ArrayBufferView): void {
    if (!isReadableByteStreamController(this)) {
      throw new TypeError("Invalid ReadableByteStreamController.");
    }
    if (this[sym.closeRequested]) {
      throw new TypeError("Closed already requested.");
    }
    if (this[sym.controlledReadableByteStream][sym.state] !== "readable") {
      throw new TypeError(
        "ReadableByteStreamController's stream is not in a readable state."
      );
    }
    if (!ArrayBuffer.isView(chunk)) {
      throw new TypeError(
        "You can only enqueue array buffer views when using a ReadableByteStreamController"
      );
    }
    if (isDetachedBuffer(chunk.buffer)) {
      throw new TypeError("Cannot enqueue a view onto a detached ArrayBuffer");
    }
    readableByteStreamControllerEnqueue(this, chunk);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  error(error?: any): void {
    if (!isReadableByteStreamController(this)) {
      throw new TypeError("Invalid ReadableByteStreamController.");
    }
    readableByteStreamControllerError(this, error);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [sym.cancelSteps](reason: any): PromiseLike<void> {
    // 3.11.5.1.1 If this.[[pendingPullIntos]] is not empty,
    resetQueue(this);
    const result = this[sym.cancelAlgorithm](reason);
    readableByteStreamControllerClearAlgorithms(this);
    return result;
  }

  [sym.pullSteps](): Promise<ReadableStreamReadResult<Uint8Array>> {
    const stream = this[sym.controlledReadableByteStream];
    assert(readableStreamHasDefaultReader(stream));
    if (this[sym.queueTotalSize] > 0) {
      assert(readableStreamGetNumReadRequests(stream) === 0);
      const entry = this[sym.queue].shift();
      assert(entry);
      this[sym.queueTotalSize] -= entry.size;
      readableByteStreamControllerHandleQueueDrain(this);
      const view = new Uint8Array(entry.value, entry.offset, entry.size);
      return Promise.resolve(
        readableStreamCreateReadResult(
          view,
          false,
          stream[sym.reader]![sym.forAuthorCode]
        )
      );
    }
    // 3.11.5.2.5 If autoAllocateChunkSize is not undefined,
    const promise = readableStreamAddReadRequest(stream);
    readableByteStreamControllerCallPullIfNeeded(this);
    return promise;
  }

  [customInspect](): string {
    return `${this.constructor.name} { byobRequest: ${String(
      this.byobRequest
    )}, desiredSize: ${String(this.desiredSize)} }`;
  }
}

setFunctionName(
  ReadableByteStreamControllerImpl,
  "ReadableByteStreamController"
);
