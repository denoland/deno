// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, runIfMain } from "./mod.ts";
import {
  assert,
  assertEquals,
  assertStrictEq,
  assertThrows,
  assertThrowsAsync
} from "./asserts.ts";

test(function testingAssertEqualActualUncoercable(): void {
  let didThrow = false;
  const a = Object.create(null);
  try {
    assertEquals(a, "bar");
  } catch (e) {
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertEqualExpectedUncoercable(): void {
  let didThrow = false;
  const a = Object.create(null);
  try {
    assertStrictEq("bar", a);
  } catch (e) {
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertStrictEqual(): void {
  const a = {};
  const b = a;
  assertStrictEq(a, b);
});

test(function testingAssertNotStrictEqual(): void {
  let didThrow = false;
  const a = {};
  const b = {};
  try {
    assertStrictEq(a, b);
  } catch (e) {
    assert(e.message === "actual: [object Object] expected: [object Object]");
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingDoesThrow(): void {
  let count = 0;
  assertThrows((): void => {
    count++;
    throw new Error();
  });
  assert(count === 1);
});

test(function testingDoesNotThrow(): void {
  let count = 0;
  let didThrow = false;
  try {
    assertThrows((): void => {
      count++;
      console.log("Hello world");
    });
  } catch (e) {
    assert(e.message === "Expected function to throw.");
    didThrow = true;
  }
  assert(count === 1);
  assert(didThrow);
});

test(function testingThrowsErrorType(): void {
  let count = 0;
  assertThrows((): void => {
    count++;
    throw new TypeError();
  }, TypeError);
  assert(count === 1);
});

test(function testingThrowsNotErrorType(): void {
  let count = 0;
  let didThrow = false;
  try {
    assertThrows((): void => {
      count++;
      throw new TypeError();
    }, RangeError);
  } catch (e) {
    assert(e.message === `Expected error to be instance of "RangeError".`);
    didThrow = true;
  }
  assert(count === 1);
  assert(didThrow);
});

test(function testingThrowsMsgIncludes(): void {
  let count = 0;
  assertThrows(
    (): void => {
      count++;
      throw new TypeError("Hello world!");
    },
    TypeError,
    "world"
  );
  assert(count === 1);
});

test(function testingThrowsMsgNotIncludes(): void {
  let count = 0;
  let didThrow = false;
  try {
    assertThrows(
      (): void => {
        count++;
        throw new TypeError("Hello world!");
      },
      TypeError,
      "foobar"
    );
  } catch (e) {
    assert(
      e.message ===
        `Expected error message to include "foobar", but got "Hello world!".`
    );
    didThrow = true;
  }
  assert(count === 1);
  assert(didThrow);
});

test(async function testingDoesThrowAsync(): Promise<void> {
  let count = 0;
  await assertThrowsAsync(
    async (): Promise<void> => {
      count++;
      throw new Error();
    }
  );
  assert(count === 1);
});

test(async function testingDoesReject(): Promise<void> {
  let count = 0;
  await assertThrowsAsync(
    (): Promise<never> => {
      count++;
      return Promise.reject(new Error());
    }
  );
  assert(count === 1);
});

test(async function testingDoesNotThrowAsync(): Promise<void> {
  let count = 0;
  let didThrow = false;
  try {
    await assertThrowsAsync(
      async (): Promise<void> => {
        count++;
        console.log("Hello world");
      }
    );
  } catch (e) {
    assert(e.message === "Expected function to throw.");
    didThrow = true;
  }
  assert(count === 1);
  assert(didThrow);
});

test(async function testingDoesNotRejectAsync(): Promise<void> {
  let count = 0;
  let didThrow = false;
  try {
    await assertThrowsAsync(
      (): Promise<void> => {
        count++;
        console.log("Hello world");
        return Promise.resolve();
      }
    );
  } catch (e) {
    assert(e.message === "Expected function to throw.");
    didThrow = true;
  }
  assert(count === 1);
  assert(didThrow);
});

test(async function testingThrowsAsyncErrorType(): Promise<void> {
  let count = 0;
  await assertThrowsAsync((): Promise<void> => {
    count++;
    throw new TypeError();
  }, TypeError);
  assert(count === 1);
});

test(async function testingThrowsAsyncNotErrorType(): Promise<void> {
  let count = 0;
  let didThrow = false;
  try {
    await assertThrowsAsync(async (): Promise<void> => {
      count++;
      throw new TypeError();
    }, RangeError);
  } catch (e) {
    assert(e.message === `Expected error to be instance of "RangeError".`);
    didThrow = true;
  }
  assert(count === 1);
  assert(didThrow);
});

test(async function testingThrowsAsyncMsgIncludes(): Promise<void> {
  let count = 0;
  await assertThrowsAsync(
    async (): Promise<void> => {
      count++;
      throw new TypeError("Hello world!");
    },
    TypeError,
    "world"
  );
  assert(count === 1);
});

test(async function testingThrowsAsyncMsgNotIncludes(): Promise<void> {
  let count = 0;
  let didThrow = false;
  try {
    await assertThrowsAsync(
      async (): Promise<void> => {
        count++;
        throw new TypeError("Hello world!");
      },
      TypeError,
      "foobar"
    );
  } catch (e) {
    assert(
      e.message ===
        `Expected error message to include "foobar", but got "Hello world!".`
    );
    didThrow = true;
  }
  assert(count === 1);
  assert(didThrow);
});

test("test fn overloading", (): void => {
  // just verifying that you can use this test definition syntax
  assert(true);
});

runIfMain(import.meta);
