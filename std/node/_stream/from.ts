// Copyright Node.js contributors. All rights reserved.

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to
// deal in the Software without restriction, including without limitation the
// rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
// sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
// IN THE SOFTWARE.
import { Buffer } from "../buffer.ts";
import Readable from "./readable.ts";
import type { ReadableOptions } from "./readable.ts";
import { ERR_INVALID_ARG_TYPE, ERR_STREAM_NULL_VALUES } from "../_errors.ts";

export default function from(
  // deno-lint-ignore no-explicit-any
  iterable: Iterable<any> | AsyncIterable<any>,
  opts?: ReadableOptions,
) {
  let iterator:
    // deno-lint-ignore no-explicit-any
    | Iterator<any, any, undefined>
    // deno-lint-ignore no-explicit-any
    | AsyncIterator<any, any, undefined>;
  if (typeof iterable === "string" || iterable instanceof Buffer) {
    return new Readable({
      objectMode: true,
      ...opts,
      read() {
        this.push(iterable);
        this.push(null);
      },
    });
  }

  if (Symbol.asyncIterator in iterable) {
    // deno-lint-ignore no-explicit-any
    iterator = (iterable as AsyncIterable<any>)[Symbol.asyncIterator]();
  } else if (Symbol.iterator in iterable) {
    // deno-lint-ignore no-explicit-any
    iterator = (iterable as Iterable<any>)[Symbol.iterator]();
  } else {
    throw new ERR_INVALID_ARG_TYPE("iterable", ["Iterable"], iterable);
  }

  const readable = new Readable({
    objectMode: true,
    highWaterMark: 1,
    ...opts,
  });

  // Reading boolean to protect against _read
  // being called before last iteration completion.
  let reading = false;

  // needToClose boolean if iterator needs to be explicitly closed
  let needToClose = false;

  readable._read = function () {
    if (!reading) {
      reading = true;
      next();
    }
  };

  readable._destroy = function (error, cb) {
    if (needToClose) {
      needToClose = false;
      close().then(
        () => queueMicrotask(() => cb(error)),
        (e) => queueMicrotask(() => cb(error || e)),
      );
    } else {
      cb(error);
    }
  };

  async function close() {
    if (typeof iterator.return === "function") {
      const { value } = await iterator.return();
      await value;
    }
  }

  async function next() {
    try {
      needToClose = false;
      const { value, done } = await iterator.next();
      needToClose = !done;
      if (done) {
        readable.push(null);
      } else if (readable.destroyed) {
        await close();
      } else {
        const res = await value;
        if (res === null) {
          reading = false;
          throw new ERR_STREAM_NULL_VALUES();
        } else if (readable.push(res)) {
          next();
        } else {
          reading = false;
        }
      }
    } catch (err) {
      readable.destroy(err);
    }
  }
  return readable;
}
