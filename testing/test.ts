// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { test, assert, assertEqual, equal } from "./mod.ts";

test(function testingEqual() {
  assert(equal("world", "world"));
  assert(!equal("hello", "world"));
  assert(equal(5, 5));
  assert(!equal(5, 6));
  assert(equal(NaN, NaN));
  assert(equal({ hello: "world" }, { hello: "world" }));
  assert(!equal({ world: "hello" }, { hello: "world" }));
  assert(
    equal(
      { hello: "world", hi: { there: "everyone" } },
      { hello: "world", hi: { there: "everyone" } }
    )
  );
  assert(
    !equal(
      { hello: "world", hi: { there: "everyone" } },
      { hello: "world", hi: { there: "everyone else" } }
    )
  );
});

test(function testingAssertEqual() {
  const a = Object.create(null);
  a.b = "foo";
  assertEqual(a, a);
  assert(assert.equal === assertEqual);
});

test(function testingAssertEqualActualUncoercable() {
  let didThrow = false;
  const a = Object.create(null);
  try {
    assertEqual(a, "bar");
  } catch (e) {
    didThrow = true;
    console.log(e.message);
    assert(e.message === "actual: [Cannot display] expected: bar");
  }
  assert(didThrow);
});

test(function testingAssertEqualExpectedUncoercable() {
  let didThrow = false;
  const a = Object.create(null);
  try {
    assertEqual("bar", a);
  } catch (e) {
    didThrow = true;
    console.log(e.message);
    assert(e.message === "actual: bar expected: [Cannot display]");
  }
  assert(didThrow);
});

test(function testingAssertStrictEqual() {
  const a = {};
  const b = a;
  assert.strictEqual(a, b);
});

test(function testingAssertNotStrictEqual() {
  let didThrow = false;
  const a = {};
  const b = {};
  try {
    assert.strictEqual(a, b);
  } catch (e) {
    assert(e.message === "actual: [object Object] expected: [object Object]");
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingDoesThrow() {
  let count = 0;
  assert.throws(() => {
    count++;
    throw new Error();
  });
  assert(count === 1);
});

test(function testingDoesNotThrow() {
  let count = 0;
  let didThrow = false;
  try {
    assert.throws(() => {
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
  assert.throws(() => {
    count++;
    throw new TypeError();
  }, TypeError);
  assert(count === 1);
});

test(function testingThrowsNotErrorType() {
  let count = 0;
  let didThrow = false;
  try {
    assert.throws(() => {
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
  assert.throws(
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
    assert.throws(
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
