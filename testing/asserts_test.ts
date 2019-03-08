// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  equal,
  assertNotEquals,
  assertStrContains,
  assertArrayContains,
  assertMatch,
  assertEquals,
  unimplemented,
  unreachable
} from "./asserts.ts";
import { test } from "./mod.ts";
// import { assertEquals as prettyAssertEqual } from "./pretty.ts";
// import "./format_test.ts";
// import "./diff_test.ts";
// import "./pretty_test.ts";

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

test(function testingNotEquals() {
  const a = { foo: "bar" };
  const b = { bar: "foo" };
  assertNotEquals(a, b);
  assertNotEquals("Denosaurus", "Tyrannosaurus");
  let didThrow;
  try {
    assertNotEquals("Raptor", "Raptor");
    didThrow = false;
  } catch (e) {
    didThrow = true;
  }
  assertEquals(didThrow, true);
});

test(function testingAssertStringContains() {
  assertStrContains("Denosaurus", "saur");
  assertStrContains("Denosaurus", "Deno");
  assertStrContains("Denosaurus", "rus");
  let didThrow;
  try {
    assertStrContains("Denosaurus", "Raptor");
    didThrow = false;
  } catch (e) {
    didThrow = true;
  }
  assertEquals(didThrow, true);
});

test(function testingArrayContains() {
  const fixture = ["deno", "iz", "luv"];
  const fixtureObject = [{ deno: "luv" }, { deno: "Js" }];
  assertArrayContains(fixture, ["deno"]);
  assertArrayContains(fixtureObject, [{ deno: "luv" }]);
  let didThrow;
  try {
    assertArrayContains(fixtureObject, [{ deno: "node" }]);
    didThrow = false;
  } catch (e) {
    didThrow = true;
  }
  assertEquals(didThrow, true);
});

test(function testingAssertStringContainsThrow() {
  let didThrow = false;
  try {
    assertStrContains("Denosaurus from Jurassic", "Raptor");
  } catch (e) {
    assert(
      e.message ===
        `actual: "Denosaurus from Jurassic" expected to contains: "Raptor"`
    );
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertStringMatching() {
  assertMatch("foobar@deno.com", RegExp(/[a-zA-Z]+@[a-zA-Z]+.com/));
});

test(function testingAssertStringMatchingThrows() {
  let didThrow = false;
  try {
    assertMatch("Denosaurus from Jurassic", RegExp(/Raptor/));
  } catch (e) {
    assert(
      e.message ===
        `actual: "Denosaurus from Jurassic" expected to match: "/Raptor/"`
    );
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertsUnimplemented() {
  let didThrow = false;
  try {
    unimplemented();
  } catch (e) {
    assert(e.message === "unimplemented");
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertsUnreachable() {
  let didThrow = false;
  try {
    unreachable();
  } catch (e) {
    assert(e.message === "unreachable");
    didThrow = true;
  }
  assert(didThrow);
});
