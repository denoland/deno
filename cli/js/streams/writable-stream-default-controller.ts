// Forked from https://github.com/stardazed/sd-streams/tree/8928cf04b035fd02fb1340b7eb541c76be37e546
// Copyright (c) 2018-Present by Arthur Langereis - @zenmumbler MIT

/**
 * streams/writable-stream-default-controller - WritableStreamDefaultController class implementation
 * Part of Stardazed
 * (c) 2018-Present by Arthur Langereis - @zenmumbler
 * https://github.com/stardazed/sd-streams
 */

import * as ws from "./writable-internals.ts";
import * as shared from "./shared-internals.ts";
import * as q from "./queue-mixin.ts";
import { Queue } from "./queue.ts";
import { QueuingStrategySizeCallback } from "../dom_types.ts";

export class WritableStreamDefaultController<InputType>
  implements ws.WritableStreamDefaultController<InputType> {
  [ws.abortAlgorithm_]: ws.AbortAlgorithm;
  [ws.closeAlgorithm_]: ws.CloseAlgorithm;
  [ws.controlledWritableStream_]: ws.WritableStream<InputType>;
  [ws.started_]: boolean;
  [ws.strategyHWM_]: number;
  [ws.strategySizeAlgorithm_]: QueuingStrategySizeCallback<InputType>;
  [ws.writeAlgorithm_]: ws.WriteAlgorithm<InputType>;

  [q.queue_]: Queue<q.QueueElement<ws.WriteRecord<InputType> | "close">>;
  [q.queueTotalSize_]: number;

  constructor() {
    throw new TypeError();
  }

  error(e?: shared.ErrorResult) {
    if (!ws.isWritableStreamDefaultController(this)) {
      throw new TypeError();
    }
    const state = this[ws.controlledWritableStream_][shared.state_];
    if (state !== "writable") {
      return;
    }
    ws.writableStreamDefaultControllerError(this, e);
  }

  [ws.abortSteps_](reason: shared.ErrorResult) {
    const result = this[ws.abortAlgorithm_](reason);
    ws.writableStreamDefaultControllerClearAlgorithms(this);
    return result;
  }

  [ws.errorSteps_]() {
    q.resetQueue(this);
  }
}

export function setUpWritableStreamDefaultControllerFromUnderlyingSink<
  InputType
>(
  stream: ws.WritableStream<InputType>,
  underlyingSink: ws.WritableStreamSink<InputType>,
  highWaterMark: number,
  sizeAlgorithm: QueuingStrategySizeCallback<InputType>
) {
  // Assert: underlyingSink is not undefined.
  const controller = Object.create(
    WritableStreamDefaultController.prototype
  ) as WritableStreamDefaultController<InputType>;

  const startAlgorithm = function() {
    return shared.invokeOrNoop(underlyingSink, "start", [controller]);
  };
  const writeAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
    underlyingSink,
    "write",
    [controller]
  );
  const closeAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
    underlyingSink,
    "close",
    []
  );
  const abortAlgorithm = shared.createAlgorithmFromUnderlyingMethod(
    underlyingSink,
    "abort",
    []
  );
  ws.setUpWritableStreamDefaultController(
    stream,
    controller,
    startAlgorithm,
    writeAlgorithm,
    closeAlgorithm,
    abortAlgorithm,
    highWaterMark,
    sizeAlgorithm
  );
}
