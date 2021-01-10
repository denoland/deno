// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
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
import {
  assert,
  assertEquals,
  assertStrictEquals,
  assertThrowsAsync,
} from "../../testing/asserts.ts";
import { promisify } from "./_util_promisify.ts";
import * as fs from "../fs.ts";

// deno-lint-ignore no-explicit-any
type VoidFunction = (...args: any[]) => void;

const readFile = promisify(fs.readFile);
const customPromisifyArgs = Symbol.for("nodejs.util.promisify.customArgs");

Deno.test(
  "Errors should reject the promise",
  async function testPromiseRejection() {
    await assertThrowsAsync(() => readFile("/dontexist"), Deno.errors.NotFound);
  },
);

Deno.test("Promisify.custom", async function testPromisifyCustom() {
  function fn(): void {}

  function promisifedFn(): void {}
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
  fn[promisify.custom] = promisifedFn;

  const promisifiedFnA = promisify(fn);
  const promisifiedFnB = promisify(promisifiedFnA);
  assertStrictEquals(promisifiedFnA, promisifedFn);
  assertStrictEquals(promisifiedFnB, promisifedFn);

  await promisifiedFnA;
  await promisifiedFnB;
});

Deno.test("promiisfy.custom symbol", function testPromisifyCustomSymbol() {
  function fn(): void {}

  function promisifiedFn(): void {}

  // util.promisify.custom is a shared symbol which can be accessed
  // as `Symbol.for("nodejs.util.promisify.custom")`.
  const kCustomPromisifiedSymbol = Symbol.for("nodejs.util.promisify.custom");
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
  fn[kCustomPromisifiedSymbol] = promisifiedFn;

  assertStrictEquals(kCustomPromisifiedSymbol, promisify.custom);
  assertStrictEquals(promisify(fn), promisifiedFn);
  assertStrictEquals(promisify(promisify(fn)), promisifiedFn);
});

Deno.test("Invalid argument should throw", function testThrowInvalidArgument() {
  function fn(): void {}
  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
  fn[promisify.custom] = 42;
  try {
    promisify(fn);
  } catch (e) {
    assertStrictEquals(e.code, "ERR_INVALID_ARG_TYPE");
    assert(e instanceof TypeError);
  }
});

Deno.test("Custom promisify args", async function testPromisifyCustomArgs() {
  const firstValue = 5;
  const secondValue = 17;

  function fn(callback: VoidFunction): void {
    callback(null, firstValue, secondValue);
  }

  // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
  fn[customPromisifyArgs] = ["first", "second"];

  const obj = await promisify(fn)();
  assertEquals(obj, { first: firstValue, second: secondValue });
});

Deno.test(
  "Multiple callback args without custom promisify args",
  async function testPromisifyWithoutCustomArgs() {
    function fn(callback: VoidFunction): void {
      callback(null, "foo", "bar");
    }
    const value = await promisify(fn)();
    assertStrictEquals(value, "foo");
  },
);

Deno.test(
  "Undefined resolved value",
  async function testPromisifyWithUndefinedResolvedValue() {
    function fn(callback: VoidFunction): void {
      callback(null);
    }
    const value = await promisify(fn)();
    assertStrictEquals(value, undefined);
  },
);

Deno.test(
  "Undefined resolved value II",
  async function testPromisifyWithUndefinedResolvedValueII() {
    function fn(callback: VoidFunction): void {
      callback();
    }
    const value = await promisify(fn)();
    assertStrictEquals(value, undefined);
  },
);

Deno.test(
  "Resolved value: number",
  async function testPromisifyWithNumberResolvedValue() {
    function fn(err: Error | null, val: number, callback: VoidFunction): void {
      callback(err, val);
    }
    const value = await promisify(fn)(null, 42);
    assertStrictEquals(value, 42);
  },
);

Deno.test(
  "Rejected value",
  async function testPromisifyWithNumberRejectedValue() {
    function fn(err: Error | null, val: null, callback: VoidFunction): void {
      callback(err, val);
    }
    await assertThrowsAsync(
      () => promisify(fn)(new Error("oops"), null),
      Error,
      "oops",
    );
  },
);

Deno.test("Rejected value", async function testPromisifyWithAsObjectMethod() {
  const o: { fn?: VoidFunction } = {};
  const fn = promisify(function (this: unknown, cb: VoidFunction): void {
    cb(null, this === o);
  });

  o.fn = fn;

  const val = await o.fn();
  assert(val);
});

Deno.test(
  "Multiple callback",
  async function testPromisifyWithMultipleCallback() {
    const err = new Error(
      "Should not have called the callback with the error.",
    );
    const stack = err.stack;

    const fn = promisify(function (cb: VoidFunction): void {
      cb(null);
      cb(err);
    });

    await fn();
    await Promise.resolve();
    return assertStrictEquals(stack, err.stack);
  },
);

Deno.test("Promisify a promise", function testPromisifyPromise() {
  function c(): void {}
  const a = promisify(function (): void {});
  const b = promisify(a);
  assert(c !== a);
  assertStrictEquals(a, b);
});

Deno.test("Test error", async function testInvalidArguments() {
  let errToThrow;

  const thrower = promisify(function (
    a: number,
    b: number,
    c: number,
    cb: VoidFunction,
  ): void {
    errToThrow = new Error(`${a}-${b}-${c}-${cb}`);
    throw errToThrow;
  });

  try {
    await thrower(1, 2, 3);
    throw new Error(`should've failed`);
  } catch (e) {
    assertStrictEquals(e, errToThrow);
  }
});

Deno.test("Test invalid arguments", function testInvalidArguments() {
  [undefined, null, true, 0, "str", {}, [], Symbol()].forEach((input) => {
    try {
      // @ts-expect-error TypeScript
      promisify(input);
    } catch (e) {
      assertStrictEquals(e.code, "ERR_INVALID_ARG_TYPE");
      assert(e instanceof TypeError);
      assertEquals(
        e.message,
        `The "original" argument must be of type Function. Received ${typeof input}`,
      );
    }
  });
});
