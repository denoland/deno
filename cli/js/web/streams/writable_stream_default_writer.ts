// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  Deferred,
  getDeferred,
  isWritableStream,
  isWritableStreamDefaultWriter,
  isWritableStreamLocked,
  setFunctionName,
  setPromiseIsHandledToTrue,
  writableStreamCloseQueuedOrInFlight,
  writableStreamDefaultWriterAbort,
  writableStreamDefaultWriterClose,
  writableStreamDefaultWriterGetDesiredSize,
  writableStreamDefaultWriterRelease,
  writableStreamDefaultWriterWrite,
} from "./internals.ts";
import * as sym from "./symbols.ts";
import { WritableStreamImpl } from "./writable_stream.ts";
import { customInspect } from "../console.ts";
import { assert } from "../../util.ts";

export class WritableStreamDefaultWriterImpl<W>
  implements WritableStreamDefaultWriter<W> {
  [sym.closedPromise]: Deferred<void>;
  [sym.ownerWritableStream]: WritableStreamImpl<W>;
  [sym.readyPromise]: Deferred<void>;

  constructor(stream: WritableStreamImpl<W>) {
    if (!isWritableStream(stream)) {
      throw new TypeError("Invalid stream.");
    }
    if (isWritableStreamLocked(stream)) {
      throw new TypeError("Cannot create a writer for a locked stream.");
    }
    this[sym.ownerWritableStream] = stream;
    stream[sym.writer] = this;
    const state = stream[sym.state];
    if (state === "writable") {
      if (
        !writableStreamCloseQueuedOrInFlight(stream) &&
        stream[sym.backpressure]
      ) {
        this[sym.readyPromise] = getDeferred();
      } else {
        this[sym.readyPromise] = { promise: Promise.resolve() };
      }
      this[sym.closedPromise] = getDeferred();
    } else if (state === "erroring") {
      this[sym.readyPromise] = {
        promise: Promise.reject(stream[sym.storedError]),
      };
      setPromiseIsHandledToTrue(this[sym.readyPromise].promise);
      this[sym.closedPromise] = getDeferred();
    } else if (state === "closed") {
      this[sym.readyPromise] = { promise: Promise.resolve() };
      this[sym.closedPromise] = { promise: Promise.resolve() };
    } else {
      assert(state === "errored");
      const storedError = stream[sym.storedError];
      this[sym.readyPromise] = { promise: Promise.reject(storedError) };
      setPromiseIsHandledToTrue(this[sym.readyPromise].promise);
      this[sym.closedPromise] = { promise: Promise.reject(storedError) };
      setPromiseIsHandledToTrue(this[sym.closedPromise].promise);
    }
  }

  get closed(): Promise<void> {
    if (!isWritableStreamDefaultWriter(this)) {
      return Promise.reject(
        new TypeError("Invalid WritableStreamDefaultWriter.")
      );
    }
    return this[sym.closedPromise].promise;
  }

  get desiredSize(): number | null {
    if (!isWritableStreamDefaultWriter(this)) {
      throw new TypeError("Invalid WritableStreamDefaultWriter.");
    }
    if (!this[sym.ownerWritableStream]) {
      throw new TypeError("WritableStreamDefaultWriter has no owner.");
    }
    return writableStreamDefaultWriterGetDesiredSize(this);
  }

  get ready(): Promise<void> {
    if (!isWritableStreamDefaultWriter(this)) {
      return Promise.reject(
        new TypeError("Invalid WritableStreamDefaultWriter.")
      );
    }
    return this[sym.readyPromise].promise;
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  abort(reason: any): Promise<void> {
    if (!isWritableStreamDefaultWriter(this)) {
      return Promise.reject(
        new TypeError("Invalid WritableStreamDefaultWriter.")
      );
    }
    if (!this[sym.ownerWritableStream]) {
      Promise.reject(
        new TypeError("WritableStreamDefaultWriter has no owner.")
      );
    }
    return writableStreamDefaultWriterAbort(this, reason);
  }

  close(): Promise<void> {
    if (!isWritableStreamDefaultWriter(this)) {
      return Promise.reject(
        new TypeError("Invalid WritableStreamDefaultWriter.")
      );
    }
    const stream = this[sym.ownerWritableStream];
    if (!stream) {
      Promise.reject(
        new TypeError("WritableStreamDefaultWriter has no owner.")
      );
    }
    if (writableStreamCloseQueuedOrInFlight(stream)) {
      Promise.reject(
        new TypeError("Stream is in an invalid state to be closed.")
      );
    }
    return writableStreamDefaultWriterClose(this);
  }

  releaseLock(): void {
    if (!isWritableStreamDefaultWriter(this)) {
      throw new TypeError("Invalid WritableStreamDefaultWriter.");
    }
    const stream = this[sym.ownerWritableStream];
    if (!stream) {
      return;
    }
    assert(stream[sym.writer]);
    writableStreamDefaultWriterRelease(this);
  }

  write(chunk: W): Promise<void> {
    if (!isWritableStreamDefaultWriter(this)) {
      return Promise.reject(
        new TypeError("Invalid WritableStreamDefaultWriter.")
      );
    }
    if (!this[sym.ownerWritableStream]) {
      Promise.reject(
        new TypeError("WritableStreamDefaultWriter has no owner.")
      );
    }
    return writableStreamDefaultWriterWrite(this, chunk);
  }

  [customInspect](): string {
    return `${this.constructor.name} { closed: Promise, desiredSize: ${String(
      this.desiredSize
    )}, ready: Promise }`;
  }
}

setFunctionName(WritableStreamDefaultWriterImpl, "WritableStreamDefaultWriter");
