// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  Deferred,
  isReadableStream,
  isReadableStreamDefaultReader,
  isReadableStreamLocked,
  readableStreamDefaultReaderRead,
  readableStreamReaderGenericCancel,
  readableStreamReaderGenericInitialize,
  readableStreamReaderGenericRelease,
  setFunctionName,
} from "./internals.ts";
import { ReadableStreamImpl } from "./readable_stream.ts";
import * as sym from "./symbols.ts";
import { customInspect } from "../console.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export class ReadableStreamDefaultReaderImpl<R = any>
  implements ReadableStreamDefaultReader<R> {
  [sym.closedPromise]: Deferred<void>;
  [sym.forAuthorCode]: boolean;
  [sym.ownerReadableStream]: ReadableStreamImpl<R>;
  [sym.readRequests]: Array<Deferred<ReadableStreamReadResult<R>>>;

  constructor(stream: ReadableStream<R>) {
    if (!isReadableStream(stream)) {
      throw new TypeError("stream is not a ReadableStream.");
    }
    if (isReadableStreamLocked(stream)) {
      throw new TypeError("stream is locked.");
    }
    readableStreamReaderGenericInitialize(this, stream);
    this[sym.readRequests] = [];
  }

  get closed(): Promise<void> {
    if (!isReadableStreamDefaultReader(this)) {
      return Promise.reject(
        new TypeError("Invalid ReadableStreamDefaultReader.")
      );
    }
    return (
      this[sym.closedPromise].promise ??
      Promise.reject(new TypeError("Invalid reader."))
    );
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  cancel(reason?: any): Promise<void> {
    if (!isReadableStreamDefaultReader(this)) {
      return Promise.reject(
        new TypeError("Invalid ReadableStreamDefaultReader.")
      );
    }
    if (!this[sym.ownerReadableStream]) {
      return Promise.reject(new TypeError("Invalid reader."));
    }
    return readableStreamReaderGenericCancel(this, reason);
  }

  read(): Promise<ReadableStreamReadResult<R>> {
    if (!isReadableStreamDefaultReader(this)) {
      return Promise.reject(
        new TypeError("Invalid ReadableStreamDefaultReader.")
      );
    }
    if (!this[sym.ownerReadableStream]) {
      return Promise.reject(new TypeError("Invalid reader."));
    }
    return readableStreamDefaultReaderRead(this);
  }

  releaseLock(): void {
    if (!isReadableStreamDefaultReader(this)) {
      throw new TypeError("Invalid ReadableStreamDefaultReader.");
    }
    if (this[sym.ownerReadableStream] === undefined) {
      return;
    }
    if (this[sym.readRequests].length) {
      throw new TypeError("Cannot release lock with pending read requests.");
    }
    readableStreamReaderGenericRelease(this);
  }

  [customInspect](): string {
    return `${this.constructor.name} { closed: Promise }`;
  }
}

setFunctionName(ReadableStreamDefaultReaderImpl, "ReadableStreamDefaultReader");
