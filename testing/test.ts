// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, runIfMain } from "./mod.ts";
import {
  assert,
  assertEquals,
  assertStrictEq,
  assertThrows,
  assertThrowsAsync
} from "../testing/asserts.ts";
import "./format_test.ts";
import "./diff_test.ts";
import "./pretty_test.ts";
import "./asserts_test.ts";
// TODO(ry) Re-enable these tests - they are causing the a hang.
// import "./bench_test.ts";

test(function testingAssertEqualActualUncoercable() {
  let didThrow = false;
  const a = Object.create(null);
  try {
    assertEquals(a, "bar");
  } catch (e) {
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertEqualExpectedUncoercable() {
  let didThrow = false;
  const a = Object.create(null);
  try {
    assertStrictEq("bar", a);
  } catch (e) {
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertStrictEqual() {
  const a = {};
  const b = a;
  assertStrictEq(a, b);
});

test(function testingAssertNotStrictEqual() {
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

test(function testingDoesThrow() {
  let count = 0;
  assertThrows(() => {
    count++;
    throw new Error();
  });
  assert(count === 1);
});

test(function testingDoesNotThrow() {
  let count = 0;
  let didThrow = false;
  try {
    assertThrows(() => {
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

test(function testingThrowsErrorType() {
  let count = 0;
  assertThrows(() => {
    count++;
    throw new TypeError();
  }, TypeError);
  assert(count === 1);
});

test(function testingThrowsNotErrorType() {
  let count = 0;
  let didThrow = false;
  try {
    assertThrows(() => {
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

test(function testingThrowsMsgIncludes() {
  let count = 0;
  assertThrows(
    () => {
      count++;
      throw new TypeError("Hello world!");
    },
    TypeError,
    "world"
  );
  assert(count === 1);
});

test(function testingThrowsMsgNotIncludes() {
  let count = 0;
  let didThrow = false;
  try {
    assertThrows(
      () => {
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

test(async function testingDoesThrowAsync() {
  let count = 0;
  await assertThrowsAsync(async () => {
    count++;
    throw new Error();
  });
  assert(count === 1);
});

test(async function testingDoesReject() {
  let count = 0;
  await assertThrowsAsync(() => {
    count++;
    return Promise.reject(new Error());
  });
  assert(count === 1);
});

test(async function testingDoesNotThrowAsync() {
  let count = 0;
  let didThrow = false;
  try {
    await assertThrowsAsync(async () => {
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

test(async function testingDoesNotRejectAsync() {
  let count = 0;
  let didThrow = false;
  try {
    await assertThrowsAsync(() => {
      count++;
      console.log("Hello world");
      return Promise.resolve();
    });
  } catch (e) {
    assert(e.message === "Expected function to throw.");
    didThrow = true;
  }
  assert(count === 1);
  assert(didThrow);
});

test(async function testingThrowsAsyncErrorType() {
  let count = 0;
  await assertThrowsAsync(async () => {
    count++;
    throw new TypeError();
  }, TypeError);
  assert(count === 1);
});

test(async function testingThrowsAsyncNotErrorType() {
  let count = 0;
  let didThrow = false;
  try {
    await assertThrowsAsync(async () => {
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

test(async function testingThrowsAsyncMsgIncludes() {
  let count = 0;
  await assertThrowsAsync(
    async () => {
      count++;
      throw new TypeError("Hello world!");
    },
    TypeError,
    "world"
  );
  assert(count === 1);
});

test(async function testingThrowsAsyncMsgNotIncludes() {
  let count = 0;
  let didThrow = false;
  try {
    await assertThrowsAsync(
      async () => {
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

runIfMain(import.meta);
