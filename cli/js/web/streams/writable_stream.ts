// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  AbortRequest,
  acquireWritableStreamDefaultWriter,
  Deferred,
  initializeWritableStream,
  isWritableStream,
  isWritableStreamLocked,
  makeSizeAlgorithmFromSizeFunction,
  setFunctionName,
  setUpWritableStreamDefaultControllerFromUnderlyingSink,
  writableStreamAbort,
  writableStreamClose,
  writableStreamCloseQueuedOrInFlight,
  validateAndNormalizeHighWaterMark,
} from "./internals.ts";
import * as sym from "./symbols.ts";
import { WritableStreamDefaultControllerImpl } from "./writable_stream_default_controller.ts";
import { WritableStreamDefaultWriterImpl } from "./writable_stream_default_writer.ts";
import { customInspect } from "../console.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export class WritableStreamImpl<W = any> implements WritableStream<W> {
  [sym.backpressure]: boolean;
  [sym.closeRequest]?: Deferred<void>;
  [sym.inFlightWriteRequest]?: Required<Deferred<void>>;
  [sym.inFlightCloseRequest]?: Deferred<void>;
  [sym.pendingAbortRequest]?: AbortRequest;
  [sym.state]: "writable" | "closed" | "erroring" | "errored";
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [sym.storedError]?: any;
  [sym.writableStreamController]?: WritableStreamDefaultControllerImpl<W>;
  [sym.writer]?: WritableStreamDefaultWriterImpl<W>;
  [sym.writeRequests]: Array<Required<Deferred<void>>>;

  constructor(
    underlyingSink: UnderlyingSink = {},
    strategy: QueuingStrategy = {}
  ) {
    initializeWritableStream(this);
    const size = strategy.size;
    let highWaterMark = strategy.highWaterMark ?? 1;
    const { type } = underlyingSink;
    if (type !== undefined) {
      throw new RangeError(`Sink type of "${String(type)}" not supported.`);
    }
    const sizeAlgorithm = makeSizeAlgorithmFromSizeFunction(size);
    highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark);
    setUpWritableStreamDefaultControllerFromUnderlyingSink(
      this,
      underlyingSink,
      highWaterMark,
      sizeAlgorithm
    );
  }

  get locked(): boolean {
    if (!isWritableStream(this)) {
      throw new TypeError("Invalid WritableStream.");
    }
    return isWritableStreamLocked(this);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  abort(reason: any): Promise<void> {
    if (!isWritableStream(this)) {
      return Promise.reject(new TypeError("Invalid WritableStream."));
    }
    if (isWritableStreamLocked(this)) {
      return Promise.reject(
        new TypeError("Cannot abort a locked WritableStream.")
      );
    }
    return writableStreamAbort(this, reason);
  }

  close(): Promise<void> {
    if (!isWritableStream(this)) {
      return Promise.reject(new TypeError("Invalid WritableStream."));
    }
    if (isWritableStreamLocked(this)) {
      return Promise.reject(
        new TypeError("Cannot abort a locked WritableStream.")
      );
    }
    if (writableStreamCloseQueuedOrInFlight(this)) {
      return Promise.reject(
        new TypeError("Cannot close an already closing WritableStream.")
      );
    }
    return writableStreamClose(this);
  }

  getWriter(): WritableStreamDefaultWriter<W> {
    if (!isWritableStream(this)) {
      throw new TypeError("Invalid WritableStream.");
    }
    return acquireWritableStreamDefaultWriter(this);
  }

  [customInspect](): string {
    return `${this.constructor.name} { locked: ${String(this.locked)} }`;
  }
}

setFunctionName(WritableStreamImpl, "WritableStream");
