// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

/* eslint-disable @typescript-eslint/no-explicit-any */
// TODO reenable this lint here

import * as rs from "./readable-internals.ts";
import * as q from "./queue-mixin.ts";
import * as shared from "./shared-internals.ts";
import { ReadableStreamBYOBRequest } from "./readable-stream-byob-request.ts";
import { Queue } from "./queue.ts";
import { UnderlyingByteSource } from "../dom_types.ts";

export class ReadableByteStreamController
  implements rs.SDReadableByteStreamController {
  [rs.autoAllocateChunkSize_]: number | undefined;
  [rs.byobRequest_]: rs.SDReadableStreamBYOBRequest | undefined;
  [rs.cancelAlgorithm_]: rs.CancelAlgorithm;
  [rs.closeRequested_]: boolean;
  [rs.controlledReadableByteStream_]: rs.SDReadableStream<ArrayBufferView>;
  [rs.pullAgain_]: boolean;
  [rs.pullAlgorithm_]: rs.PullAlgorithm<ArrayBufferView>;
  [rs.pulling_]: boolean;
  [rs.pendingPullIntos_]: rs.PullIntoDescriptor[];
  [rs.started_]: boolean;
  [rs.strategyHWM_]: number;

  [q.queue_]: Queue<{
    buffer: ArrayBufferLike;
    byteOffset: number;
    byteLength: number;
  }>;
  [q.queueTotalSize_]: number;

  constructor() {
    throw new TypeError();
  }

  get byobRequest(): rs.SDReadableStreamBYOBRequest | undefined {
    if (!rs.isReadableByteStreamController(this)) {
      throw new TypeError();
    }
    if (
      this[rs.byobRequest_] === undefined &&
      this[rs.pendingPullIntos_].length > 0
    ) {
      const firstDescriptor = this[rs.pendingPullIntos_][0];
      const view = new Uint8Array(
        firstDescriptor.buffer,
        firstDescriptor.byteOffset + firstDescriptor.bytesFilled,
        firstDescriptor.byteLength - firstDescriptor.bytesFilled
      );
      const byobRequest = Object.create(
        ReadableStreamBYOBRequest.prototype
      ) as ReadableStreamBYOBRequest;
      rs.setUpReadableStreamBYOBRequest(byobRequest, this, view);
      this[rs.byobRequest_] = byobRequest;
    }
    return this[rs.byobRequest_];
  }

  get desiredSize(): number | null {
    if (!rs.isReadableByteStreamController(this)) {
      throw new TypeError();
    }
    return rs.readableByteStreamControllerGetDesiredSize(this);
  }

  close(): void {
    if (!rs.isReadableByteStreamController(this)) {
      throw new TypeError();
    }
    if (this[rs.closeRequested_]) {
      throw new TypeError("Stream is already closing");
    }
    if (this[rs.controlledReadableByteStream_][shared.state_] !== "readable") {
      throw new TypeError("Stream is closed or errored");
    }
    rs.readableByteStreamControllerClose(this);
  }

  enqueue(chunk: ArrayBufferView): void {
    if (!rs.isReadableByteStreamController(this)) {
      throw new TypeError();
    }
    if (this[rs.closeRequested_]) {
      throw new TypeError("Stream is already closing");
    }
    if (this[rs.controlledReadableByteStream_][shared.state_] !== "readable") {
      throw new TypeError("Stream is closed or errored");
    }
    if (!ArrayBuffer.isView(chunk)) {
      throw new TypeError("chunk must be a valid ArrayBufferView");
    }
    // If ! IsDetachedBuffer(chunk.[[ViewedArrayBuffer]]) is true, throw a TypeError exception.
    return rs.readableByteStreamControllerEnqueue(this, chunk);
  }

  error(error?: shared.ErrorResult): void {
    if (!rs.isReadableByteStreamController(this)) {
      throw new TypeError();
    }
    rs.readableByteStreamControllerError(this, error);
  }

  [rs.cancelSteps_](reason: shared.ErrorResult): Promise<void> {
    if (this[rs.pendingPullIntos_].length > 0) {
      const firstDescriptor = this[rs.pendingPullIntos_][0];
      firstDescriptor.bytesFilled = 0;
    }
    q.resetQueue(this);
    const result = this[rs.cancelAlgorithm_](reason);
    rs.readableByteStreamControllerClearAlgorithms(this);
    return result;
  }

  [rs.pullSteps_](
    forAuthorCode: boolean
  ): Promise<IteratorResult<ArrayBufferView, any>> {
    const stream = this[rs.controlledReadableByteStream_];
    // Assert: ! ReadableStreamHasDefaultReader(stream) is true.
    if (this[q.queueTotalSize_] > 0) {
      // Assert: ! ReadableStreamGetNumReadRequests(stream) is 0.
      const entry = this[q.queue_].shift()!;
      this[q.queueTotalSize_] -= entry.byteLength;
      rs.readableByteStreamControllerHandleQueueDrain(this);
      const view = new Uint8Array(
        entry.buffer,
        entry.byteOffset,
        entry.byteLength
      );
      return Promise.resolve(
        rs.readableStreamCreateReadResult(view, false, forAuthorCode)
      );
    }
    const autoAllocateChunkSize = this[rs.autoAllocateChunkSize_];
    if (autoAllocateChunkSize !== undefined) {
      let buffer: ArrayBuffer;
      try {
        buffer = new ArrayBuffer(autoAllocateChunkSize);
      } catch (error) {
        return Promise.reject(error);
      }
      const pullIntoDescriptor: rs.PullIntoDescriptor = {
        buffer,
        byteOffset: 0,
        byteLength: autoAllocateChunkSize,
        bytesFilled: 0,
        elementSize: 1,
        ctor: Uint8Array,
        readerType: "default",
      };
      this[rs.pendingPullIntos_].push(pullIntoDescriptor);
    }

    const promise = rs.readableStreamAddReadRequest(stream, forAuthorCode);
    rs.readableByteStreamControllerCallPullIfNeeded(this);
    return promise;
  }
}

export function setUpReadableByteStreamControllerFromUnderlyingSource(
  stream: rs.SDReadableStream<ArrayBufferView>,
  underlyingByteSource: UnderlyingByteSource,
  highWaterMark: number
): void {
  // Assert: underlyingByteSource is not undefined.
  const controller = Object.create(
    ReadableByteStreamController.prototype
  ) as ReadableByteStreamController;

  const startAlgorithm = (): any => {
    return shared.invokeOrNoop(underlyingByteSource, "start", [controller]);
  };
  const pullAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
    underlyingByteSource,
    "pull",
    [controller]
  );
  const cancelAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
    underlyingByteSource,
    "cancel",
    []
  );

  let autoAllocateChunkSize = underlyingByteSource.autoAllocateChunkSize;
  if (autoAllocateChunkSize !== undefined) {
    autoAllocateChunkSize = Number(autoAllocateChunkSize);
    if (
      !shared.isInteger(autoAllocateChunkSize) ||
      autoAllocateChunkSize <= 0
    ) {
      throw new RangeError(
        "autoAllocateChunkSize must be a positive, finite integer"
      );
    }
  }
  rs.setUpReadableByteStreamController(
    stream,
    controller,
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark,
    autoAllocateChunkSize
  );
}
