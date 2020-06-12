// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
//
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.
import { assert, assertStrictEquals } from "../../testing/asserts.ts";
import { callbackify } from "./_util_callbackify.ts";

const values = [
  "hello world",
  null,
  undefined,
  false,
  0,
  {},
  { key: "value" },
  Symbol("I am a symbol"),
  function ok(): void {},
  ["array", "with", 4, "values"],
  new Error("boo"),
];

class TestQueue {
  #waitingPromise: Promise<void>;
  #resolve?: () => void;
  #reject?: (err: unknown) => void;
  #queueSize = 0;

  constructor() {
    this.#waitingPromise = new Promise((resolve, reject) => {
      this.#resolve = resolve;
      this.#reject = reject;
    });
  }

  enqueue(fn: (done: () => void) => void): void {
    this.#queueSize++;
    try {
      fn(() => {
        this.#queueSize--;
        if (this.#queueSize === 0) {
          assert(
            this.#resolve,
            "Test setup error; async queue is missing #resolve"
          );
          this.#resolve();
        }
      });
    } catch (err) {
      assert(this.#reject, "Test setup error; async queue is missing #reject");
      this.#reject(err);
    }
  }

  waitForCompletion(): Promise<void> {
    return this.#waitingPromise;
  }
}

Deno.test(
  "callbackify passes the resolution value as the second argument to the callback",
  async () => {
    const testQueue = new TestQueue();

    for (const value of values) {
      // eslint-disable-next-line require-await
      async function asyncFn(): Promise<typeof value> {
        return value;
      }
      const cbAsyncFn = callbackify(asyncFn);
      testQueue.enqueue((done) => {
        cbAsyncFn((err: unknown, ret: unknown) => {
          assertStrictEquals(err, null);
          assertStrictEquals(ret, value);
          done();
        });
      });

      function promiseFn(): Promise<typeof value> {
        return Promise.resolve(value);
      }
      const cbPromiseFn = callbackify(promiseFn);
      testQueue.enqueue((done) => {
        cbPromiseFn((err: unknown, ret: unknown) => {
          assertStrictEquals(err, null);
          assertStrictEquals(ret, value);
          done();
        });
      });

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      function thenableFn(): PromiseLike<any> {
        return {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          then(onfulfilled): PromiseLike<any> {
            assert(onfulfilled);
            onfulfilled(value);
            return this;
          },
        };
      }
      const cbThenableFn = callbackify(thenableFn);
      testQueue.enqueue((done) => {
        cbThenableFn((err: unknown, ret: unknown) => {
          assertStrictEquals(err, null);
          assertStrictEquals(ret, value);
          done();
        });
      });
    }

    await testQueue.waitForCompletion();
  }
);

Deno.test(
  "callbackify passes the rejection value as the first argument to the callback",
  async () => {
    const testQueue = new TestQueue();

    for (const value of values) {
      // eslint-disable-next-line require-await
      async function asyncFn(): Promise<never> {
        return Promise.reject(value);
      }
      const cbAsyncFn = callbackify(asyncFn);
      assertStrictEquals(cbAsyncFn.length, 1);
      assertStrictEquals(cbAsyncFn.name, "asyncFnCallbackified");
      testQueue.enqueue((done) => {
        cbAsyncFn((err: unknown, ret: unknown) => {
          assertStrictEquals(ret, undefined);
          if (err instanceof Error) {
            if ("reason" in err) {
              assert(!value);
              assertStrictEquals(
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                (err as any).code,
                "ERR_FALSY_VALUE_REJECTION"
              );
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              assertStrictEquals((err as any).reason, value);
            } else {
              assertStrictEquals(String(value).endsWith(err.message), true);
            }
          } else {
            assertStrictEquals(err, value);
          }
          done();
        });
      });

      function promiseFn(): Promise<never> {
        return Promise.reject(value);
      }
      const obj = {};
      Object.defineProperty(promiseFn, "name", {
        value: obj,
        writable: false,
        enumerable: false,
        configurable: true,
      });

      const cbPromiseFn = callbackify(promiseFn);
      assertStrictEquals(promiseFn.name, obj);
      testQueue.enqueue((done) => {
        cbPromiseFn((err: unknown, ret: unknown) => {
          assertStrictEquals(ret, undefined);
          if (err instanceof Error) {
            if ("reason" in err) {
              assert(!value);
              assertStrictEquals(
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                (err as any).code,
                "ERR_FALSY_VALUE_REJECTION"
              );
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              assertStrictEquals((err as any).reason, value);
            } else {
              assertStrictEquals(String(value).endsWith(err.message), true);
            }
          } else {
            assertStrictEquals(err, value);
          }
          done();
        });
      });

      function thenableFn(): PromiseLike<never> {
        return {
          then(onfulfilled, onrejected): PromiseLike<never> {
            assert(onrejected);
            onrejected(value);
            return this;
          },
        };
      }

      const cbThenableFn = callbackify(thenableFn);
      testQueue.enqueue((done) => {
        cbThenableFn((err: unknown, ret: unknown) => {
          assertStrictEquals(ret, undefined);
          if (err instanceof Error) {
            if ("reason" in err) {
              assert(!value);
              assertStrictEquals(
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                (err as any).code,
                "ERR_FALSY_VALUE_REJECTION"
              );
              // eslint-disable-next-line @typescript-eslint/no-explicit-any
              assertStrictEquals((err as any).reason, value);
            } else {
              assertStrictEquals(String(value).endsWith(err.message), true);
            }
          } else {
            assertStrictEquals(err, value);
          }
          done();
        });
      });
    }

    await testQueue.waitForCompletion();
  }
);

Deno.test("callbackify passes arguments to the original", async () => {
  const testQueue = new TestQueue();

  for (const value of values) {
    // eslint-disable-next-line require-await
    async function asyncFn<T>(arg: T): Promise<T> {
      assertStrictEquals(arg, value);
      return arg;
    }

    const cbAsyncFn = callbackify(asyncFn);
    assertStrictEquals(cbAsyncFn.length, 2);
    assert(Object.getPrototypeOf(cbAsyncFn) !== Object.getPrototypeOf(asyncFn));
    assertStrictEquals(Object.getPrototypeOf(cbAsyncFn), Function.prototype);
    testQueue.enqueue((done) => {
      cbAsyncFn(value, (err: unknown, ret: unknown) => {
        assertStrictEquals(err, null);
        assertStrictEquals(ret, value);
        done();
      });
    });

    function promiseFn<T>(arg: T): Promise<T> {
      assertStrictEquals(arg, value);
      return Promise.resolve(arg);
    }
    const obj = {};
    Object.defineProperty(promiseFn, "length", {
      value: obj,
      writable: false,
      enumerable: false,
      configurable: true,
    });

    const cbPromiseFn = callbackify(promiseFn);
    assertStrictEquals(promiseFn.length, obj);
    testQueue.enqueue((done) => {
      cbPromiseFn(value, (err: unknown, ret: unknown) => {
        assertStrictEquals(err, null);
        assertStrictEquals(ret, value);
        done();
      });
    });
  }

  await testQueue.waitForCompletion();
});

Deno.test("callbackify preserves the `this` binding", async () => {
  const testQueue = new TestQueue();

  for (const value of values) {
    const objectWithSyncFunction = {
      fn(this: unknown, arg: typeof value): Promise<typeof value> {
        assertStrictEquals(this, objectWithSyncFunction);
        return Promise.resolve(arg);
      },
    };
    const cbSyncFunction = callbackify(objectWithSyncFunction.fn);
    testQueue.enqueue((done) => {
      cbSyncFunction.call(objectWithSyncFunction, value, function (
        this: unknown,
        err: unknown,
        ret: unknown
      ) {
        assertStrictEquals(err, null);
        assertStrictEquals(ret, value);
        assertStrictEquals(this, objectWithSyncFunction);
        done();
      });
    });

    const objectWithAsyncFunction = {
      // eslint-disable-next-line require-await
      async fn(this: unknown, arg: typeof value): Promise<typeof value> {
        assertStrictEquals(this, objectWithAsyncFunction);
        return arg;
      },
    };
    const cbAsyncFunction = callbackify(objectWithAsyncFunction.fn);
    testQueue.enqueue((done) => {
      cbAsyncFunction.call(objectWithAsyncFunction, value, function (
        this: unknown,
        err: unknown,
        ret: unknown
      ) {
        assertStrictEquals(err, null);
        assertStrictEquals(ret, value);
        assertStrictEquals(this, objectWithAsyncFunction);
        done();
      });
    });
  }

  await testQueue.waitForCompletion();
});

Deno.test("callbackify throws with non-function inputs", () => {
  ["foo", null, undefined, false, 0, {}, Symbol(), []].forEach((value) => {
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      callbackify(value as any);
      throw Error("We should never reach this error");
    } catch (err) {
      assert(err instanceof TypeError);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      assertStrictEquals((err as any).code, "ERR_INVALID_ARG_TYPE");
      assertStrictEquals(err.name, "TypeError");
      assertStrictEquals(
        err.message,
        'The "original" argument must be of type function.'
      );
    }
  });
});

Deno.test(
  "callbackify returns a function that throws if the last argument is not a function",
  () => {
    // eslint-disable-next-line require-await
    async function asyncFn(): Promise<number> {
      return 42;
    }

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const cb = callbackify(asyncFn) as any;
    const args: unknown[] = [];

    ["foo", null, undefined, false, 0, {}, Symbol(), []].forEach((value) => {
      args.push(value);

      try {
        cb(...args);
        throw Error("We should never reach this error");
      } catch (err) {
        assert(err instanceof TypeError);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        assertStrictEquals((err as any).code, "ERR_INVALID_ARG_TYPE");
        assertStrictEquals(err.name, "TypeError");
        assertStrictEquals(
          err.message,
          "The last argument must be of type function."
        );
      }
    });
  }
);
