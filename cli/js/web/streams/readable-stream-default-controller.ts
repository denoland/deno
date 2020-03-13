// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

/* eslint-disable @typescript-eslint/no-explicit-any */
// TODO reenable this lint here

import * as rs from "./readable-internals.ts";
import * as shared from "./shared-internals.ts";
import * as q from "./queue-mixin.ts";
import { Queue } from "./queue.ts";
import { QueuingStrategySizeCallback, UnderlyingSource } from "../dom_types.ts";

export class ReadableStreamDefaultController<OutputType>
  implements rs.SDReadableStreamDefaultController<OutputType> {
  [rs.cancelAlgorithm_]: rs.CancelAlgorithm;
  [rs.closeRequested_]: boolean;
  [rs.controlledReadableStream_]: rs.SDReadableStream<OutputType>;
  [rs.pullAgain_]: boolean;
  [rs.pullAlgorithm_]: rs.PullAlgorithm<OutputType>;
  [rs.pulling_]: boolean;
  [rs.strategyHWM_]: number;
  [rs.strategySizeAlgorithm_]: QueuingStrategySizeCallback<OutputType>;
  [rs.started_]: boolean;

  [q.queue_]: Queue<q.QueueElement<OutputType>>;
  [q.queueTotalSize_]: number;

  constructor() {
    throw new TypeError();
  }

  get desiredSize(): number | null {
    return rs.readableStreamDefaultControllerGetDesiredSize(this);
  }

  close(): void {
    if (!rs.isReadableStreamDefaultController(this)) {
      throw new TypeError();
    }
    if (!rs.readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
      throw new TypeError(
        "Cannot close, the stream is already closing or not readable"
      );
    }
    rs.readableStreamDefaultControllerClose(this);
  }

  enqueue(chunk?: OutputType): void {
    if (!rs.isReadableStreamDefaultController(this)) {
      throw new TypeError();
    }
    if (!rs.readableStreamDefaultControllerCanCloseOrEnqueue(this)) {
      throw new TypeError(
        "Cannot enqueue, the stream is closing or not readable"
      );
    }
    rs.readableStreamDefaultControllerEnqueue(this, chunk!);
  }

  error(e?: shared.ErrorResult): void {
    if (!rs.isReadableStreamDefaultController(this)) {
      throw new TypeError();
    }
    rs.readableStreamDefaultControllerError(this, e);
  }

  [rs.cancelSteps_](reason: shared.ErrorResult): Promise<void> {
    q.resetQueue(this);
    const result = this[rs.cancelAlgorithm_](reason);
    rs.readableStreamDefaultControllerClearAlgorithms(this);
    return result;
  }

  [rs.pullSteps_](
    forAuthorCode: boolean
  ): Promise<IteratorResult<OutputType, any>> {
    const stream = this[rs.controlledReadableStream_];
    if (this[q.queue_].length > 0) {
      const chunk = q.dequeueValue(this);
      if (this[rs.closeRequested_] && this[q.queue_].length === 0) {
        rs.readableStreamDefaultControllerClearAlgorithms(this);
        rs.readableStreamClose(stream);
      } else {
        rs.readableStreamDefaultControllerCallPullIfNeeded(this);
      }
      return Promise.resolve(
        rs.readableStreamCreateReadResult(chunk, false, forAuthorCode)
      );
    }

    const pendingPromise = rs.readableStreamAddReadRequest(
      stream,
      forAuthorCode
    );
    rs.readableStreamDefaultControllerCallPullIfNeeded(this);
    return pendingPromise;
  }
}

export function setUpReadableStreamDefaultControllerFromUnderlyingSource<
  OutputType
>(
  stream: rs.SDReadableStream<OutputType>,
  underlyingSource: UnderlyingSource<OutputType>,
  highWaterMark: number,
  sizeAlgorithm: QueuingStrategySizeCallback<OutputType>
): void {
  // Assert: underlyingSource is not undefined.
  const controller = Object.create(ReadableStreamDefaultController.prototype);
  const startAlgorithm = (): any => {
    return shared.invokeOrNoop(underlyingSource, "start", [controller]);
  };
  const pullAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
    underlyingSource,
    "pull",
    [controller]
  );
  const cancelAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
    underlyingSource,
    "cancel",
    []
  );
  rs.setUpReadableStreamDefaultController(
    stream,
    controller,
    startAlgorithm,
    pullAlgorithm,
    cancelAlgorithm,
    highWaterMark,
    sizeAlgorithm
  );
}
