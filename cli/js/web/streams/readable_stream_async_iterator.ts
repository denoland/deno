// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import * as sym from "./symbols.ts";
import {
  isReadableStreamAsyncIterator,
  ReadableStreamAsyncIterator,
  readableStreamCreateReadResult,
  readableStreamReaderGenericCancel,
  readableStreamReaderGenericRelease,
  readableStreamDefaultReaderRead,
} from "./internals.ts";
import { assert } from "../../util.ts";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const AsyncIteratorPrototype: AsyncIterableIterator<any> = Object.getPrototypeOf(
  Object.getPrototypeOf(async function* () {}).prototype
);

export const ReadableStreamAsyncIteratorPrototype: ReadableStreamAsyncIterator = Object.setPrototypeOf(
  {
    next(
      this: ReadableStreamAsyncIterator
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    ): Promise<ReadableStreamReadResult<any>> {
      if (!isReadableStreamAsyncIterator(this)) {
        return Promise.reject(
          new TypeError("invalid ReadableStreamAsyncIterator.")
        );
      }
      const reader = this[sym.asyncIteratorReader];
      if (!reader[sym.ownerReadableStream]) {
        return Promise.reject(
          new TypeError("reader owner ReadableStream is undefined.")
        );
      }
      return readableStreamDefaultReaderRead(reader).then((result) => {
        assert(typeof result === "object");
        const { done } = result;
        assert(typeof done === "boolean");
        if (done) {
          readableStreamReaderGenericRelease(reader);
        }
        const { value } = result;
        return readableStreamCreateReadResult(value, done, true);
      });
    },
    return(
      this: ReadableStreamAsyncIterator,
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      value?: any | PromiseLike<any>
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    ): Promise<ReadableStreamReadResult<any>> {
      if (!isReadableStreamAsyncIterator(this)) {
        return Promise.reject(
          new TypeError("invalid ReadableStreamAsyncIterator.")
        );
      }
      const reader = this[sym.asyncIteratorReader];
      if (!reader[sym.ownerReadableStream]) {
        return Promise.reject(
          new TypeError("reader owner ReadableStream is undefined.")
        );
      }
      if (reader[sym.readRequests].length) {
        return Promise.reject(
          new TypeError("reader has outstanding read requests.")
        );
      }
      if (!this[sym.preventCancel]) {
        const result = readableStreamReaderGenericCancel(reader, value);
        readableStreamReaderGenericRelease(reader);
        return result.then(() =>
          readableStreamCreateReadResult(value, true, true)
        );
      }
      readableStreamReaderGenericRelease(reader);
      return Promise.resolve(readableStreamCreateReadResult(value, true, true));
    },
  },
  AsyncIteratorPrototype
);
