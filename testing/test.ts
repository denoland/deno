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
