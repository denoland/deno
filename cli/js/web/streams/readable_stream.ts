// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  acquireReadableStreamDefaultReader,
  initializeReadableStream,
  isReadableStream,
  isReadableStreamLocked,
  isUnderlyingByteSource,
  isWritableStream,
  isWritableStreamLocked,
  makeSizeAlgorithmFromSizeFunction,
  setFunctionName,
  setPromiseIsHandledToTrue,
  readableStreamCancel,
  ReadableStreamGenericReader,
  readableStreamPipeTo,
  readableStreamTee,
  setUpReadableByteStreamControllerFromUnderlyingSource,
  setUpReadableStreamDefaultControllerFromUnderlyingSource,
  validateAndNormalizeHighWaterMark,
} from "./internals.ts";
import { ReadableByteStreamControllerImpl } from "./readable_byte_stream_controller.ts";
import { ReadableStreamAsyncIteratorPrototype } from "./readable_stream_async_iterator.ts";
import { ReadableStreamDefaultControllerImpl } from "./readable_stream_default_controller.ts";
import * as sym from "./symbols.ts";
import { customInspect } from "../console.ts";
import { AbortSignalImpl } from "../abort_signal.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export class ReadableStreamImpl<R = any> implements ReadableStream<R> {
  [sym.disturbed]: boolean;
  [sym.readableStreamController]:
    | ReadableStreamDefaultControllerImpl<R>
    | ReadableByteStreamControllerImpl;
  [sym.reader]: ReadableStreamGenericReader<R> | undefined;
  [sym.state]: "readable" | "closed" | "errored";
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [sym.storedError]: any;

  constructor(
    underlyingSource: UnderlyingByteSource | UnderlyingSource<R> = {},
    strategy:
      | {
          highWaterMark?: number;
          size?: undefined;
        }
      | QueuingStrategy<R> = {}
  ) {
    initializeReadableStream(this);
    const { size } = strategy;
    let { highWaterMark } = strategy;
    const { type } = underlyingSource;

    if (isUnderlyingByteSource(underlyingSource)) {
      if (size !== undefined) {
        throw new RangeError(
          `When underlying source is "bytes", strategy.size must be undefined.`
        );
      }
      highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark ?? 0);
      setUpReadableByteStreamControllerFromUnderlyingSource(
        this,
        underlyingSource,
        highWaterMark
      );
    } else if (type === undefined) {
      const sizeAlgorithm = makeSizeAlgorithmFromSizeFunction(size);
      highWaterMark = validateAndNormalizeHighWaterMark(highWaterMark ?? 1);
      setUpReadableStreamDefaultControllerFromUnderlyingSource(
        this,
        underlyingSource,
        highWaterMark,
        sizeAlgorithm
      );
    } else {
      throw new RangeError(
        `Valid values for underlyingSource are "bytes" or undefined.  Received: "${type}".`
      );
    }
  }

  get locked(): boolean {
    if (!isReadableStream(this)) {
      throw new TypeError("Invalid ReadableStream.");
    }
    return isReadableStreamLocked(this);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  cancel(reason?: any): Promise<void> {
    if (!isReadableStream(this)) {
      return Promise.reject(new TypeError("Invalid ReadableStream."));
    }
    if (isReadableStreamLocked(this)) {
      return Promise.reject(
        new TypeError("Cannot cancel a locked ReadableStream.")
      );
    }
    return readableStreamCancel(this, reason);
  }

  getIterator({
    preventCancel,
  }: { preventCancel?: boolean } = {}): AsyncIterableIterator<R> {
    if (!isReadableStream(this)) {
      throw new TypeError("Invalid ReadableStream.");
    }
    const reader = acquireReadableStreamDefaultReader(this);
    const iterator = Object.create(ReadableStreamAsyncIteratorPrototype);
    iterator[sym.asyncIteratorReader] = reader;
    iterator[sym.preventCancel] = Boolean(preventCancel);
    return iterator;
  }

  getReader({ mode }: { mode?: string } = {}): ReadableStreamDefaultReader<R> {
    if (!isReadableStream(this)) {
      throw new TypeError("Invalid ReadableStream.");
    }
    if (mode === undefined) {
      return acquireReadableStreamDefaultReader(this, true);
    }
    mode = String(mode);
    // 3.2.5.4.4 If mode is "byob", return ? AcquireReadableStreamBYOBReader(this, true).
    throw new RangeError(`Unsupported mode "${mode}"`);
  }

  pipeThrough<T>(
    {
      writable,
      readable,
    }: {
      writable: WritableStream<R>;
      readable: ReadableStream<T>;
    },
    { preventClose, preventAbort, preventCancel, signal }: PipeOptions = {}
  ): ReadableStream<T> {
    if (!isReadableStream(this)) {
      throw new TypeError("Invalid ReadableStream.");
    }
    if (!isWritableStream(writable)) {
      throw new TypeError("writable is not a valid WritableStream.");
    }
    if (!isReadableStream(readable)) {
      throw new TypeError("readable is not a valid ReadableStream.");
    }
    preventClose = Boolean(preventClose);
    preventAbort = Boolean(preventAbort);
    preventCancel = Boolean(preventCancel);
    if (signal && !(signal instanceof AbortSignalImpl)) {
      throw new TypeError("Invalid signal.");
    }
    if (isReadableStreamLocked(this)) {
      throw new TypeError("ReadableStream is locked.");
    }
    if (isWritableStreamLocked(writable)) {
      throw new TypeError("writable is locked.");
    }
    const promise = readableStreamPipeTo(
      this,
      writable,
      preventClose,
      preventAbort,
      preventCancel,
      signal
    );
    setPromiseIsHandledToTrue(promise);
    return readable;
  }

  pipeTo(
    dest: WritableStream<R>,
    { preventClose, preventAbort, preventCancel, signal }: PipeOptions = {}
  ): Promise<void> {
    if (!isReadableStream(this)) {
      return Promise.reject(new TypeError("Invalid ReadableStream."));
    }
    if (!isWritableStream(dest)) {
      return Promise.reject(
        new TypeError("dest is not a valid WritableStream.")
      );
    }
    preventClose = Boolean(preventClose);
    preventAbort = Boolean(preventAbort);
    preventCancel = Boolean(preventCancel);
    if (signal && !(signal instanceof AbortSignalImpl)) {
      return Promise.reject(new TypeError("Invalid signal."));
    }
    if (isReadableStreamLocked(this)) {
      return Promise.reject(new TypeError("ReadableStream is locked."));
    }
    if (isWritableStreamLocked(dest)) {
      return Promise.reject(new TypeError("dest is locked."));
    }
    return readableStreamPipeTo(
      this,
      dest,
      preventClose,
      preventAbort,
      preventCancel,
      signal
    );
  }

  tee(): [ReadableStreamImpl<R>, ReadableStreamImpl<R>] {
    if (!isReadableStream(this)) {
      throw new TypeError("Invalid ReadableStream.");
    }
    return readableStreamTee(this, false);
  }

  [customInspect](): string {
    return `${this.constructor.name} { locked: ${String(this.locked)} }`;
  }

  [Symbol.asyncIterator](
    options: {
      preventCancel?: boolean;
    } = {}
  ): AsyncIterableIterator<R> {
    return this.getIterator(options);
  }
}

setFunctionName(ReadableStreamImpl, "ReadableStream");
