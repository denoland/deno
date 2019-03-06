// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { assert, equal, assertStrContains, assertMatch } from "./asserts.ts";
import { test } from "./mod.ts";
// import { assertEq as prettyAssertEqual } from "./pretty.ts";
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

test(function testingAssertStringContains() {
  assertStrContains("Denosaurus", "saur");
  assertStrContains("Denosaurus", "Deno");
  assertStrContains("Denosaurus", "rus");
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
