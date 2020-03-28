// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertNotEquals,
  assertStrContains,
  assertArrayContains,
  assertMatch,
  assertEquals,
  assertThrows,
  AssertionError,
  equal,
  fail,
  unimplemented,
  unreachable,
} from "./asserts.ts";
import { red, green, white, gray, bold } from "../fmt/colors.ts";
const { test } = Deno;

test(function testingEqual(): void {
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
  assert(equal(/deno/, /deno/));
  assert(!equal(/deno/, /node/));
  assert(equal(new Date(2019, 0, 3), new Date(2019, 0, 3)));
  assert(!equal(new Date(2019, 0, 3), new Date(2019, 1, 3)));
  assert(equal(new Set([1]), new Set([1])));
  assert(!equal(new Set([1]), new Set([2])));
  assert(equal(new Set([1, 2, 3]), new Set([3, 2, 1])));
  assert(equal(new Set([1, new Set([2, 3])]), new Set([new Set([3, 2]), 1])));
  assert(!equal(new Set([1, 2]), new Set([3, 2, 1])));
  assert(!equal(new Set([1, 2, 3]), new Set([4, 5, 6])));
  assert(equal(new Set("denosaurus"), new Set("denosaurussss")));
  assert(equal(new Map(), new Map()));
  assert(
    equal(
      new Map([
        ["foo", "bar"],
        ["baz", "baz"],
      ]),
      new Map([
        ["foo", "bar"],
        ["baz", "baz"],
      ])
    )
  );
  assert(
    equal(
      new Map([["foo", new Map([["bar", "baz"]])]]),
      new Map([["foo", new Map([["bar", "baz"]])]])
    )
  );
  assert(
    equal(
      new Map([["foo", { bar: "baz" }]]),
      new Map([["foo", { bar: "baz" }]])
    )
  );
  assert(
    equal(
      new Map([
        ["foo", "bar"],
        ["baz", "qux"],
      ]),
      new Map([
        ["baz", "qux"],
        ["foo", "bar"],
      ])
    )
  );
  assert(equal(new Map([["foo", ["bar"]]]), new Map([["foo", ["bar"]]])));
  assert(!equal(new Map([["foo", "bar"]]), new Map([["bar", "baz"]])));
  assert(
    !equal(
      new Map([["foo", "bar"]]),
      new Map([
        ["foo", "bar"],
        ["bar", "baz"],
      ])
    )
  );
  assert(
    !equal(
      new Map([["foo", new Map([["bar", "baz"]])]]),
      new Map([["foo", new Map([["bar", "qux"]])]])
    )
  );
  assert(equal(new Map([[{ x: 1 }, true]]), new Map([[{ x: 1 }, true]])));
  assert(!equal(new Map([[{ x: 1 }, true]]), new Map([[{ x: 1 }, false]])));
  assert(!equal(new Map([[{ x: 1 }, true]]), new Map([[{ x: 2 }, true]])));
  assert(equal([1, 2, 3], [1, 2, 3]));
  assert(equal([1, [2, 3]], [1, [2, 3]]));
  assert(!equal([1, 2, 3, 4], [1, 2, 3]));
  assert(!equal([1, 2, 3, 4], [1, 2, 3]));
  assert(!equal([1, 2, 3, 4], [1, 4, 2, 3]));
  assert(equal(new Uint8Array([1, 2, 3, 4]), new Uint8Array([1, 2, 3, 4])));
  assert(!equal(new Uint8Array([1, 2, 3, 4]), new Uint8Array([2, 1, 4, 3])));
});

test(function testingNotEquals(): void {
  const a = { foo: "bar" };
  const b = { bar: "foo" };
  assertNotEquals(a, b);
  assertNotEquals("Denosaurus", "Tyrannosaurus");
  let didThrow;
  try {
    assertNotEquals("Raptor", "Raptor");
    didThrow = false;
  } catch (e) {
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assertEquals(didThrow, true);
});

test(function testingAssertStringContains(): void {
  assertStrContains("Denosaurus", "saur");
  assertStrContains("Denosaurus", "Deno");
  assertStrContains("Denosaurus", "rus");
  let didThrow;
  try {
    assertStrContains("Denosaurus", "Raptor");
    didThrow = false;
  } catch (e) {
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assertEquals(didThrow, true);
});

test(function testingArrayContains(): void {
  const fixture = ["deno", "iz", "luv"];
  const fixtureObject = [{ deno: "luv" }, { deno: "Js" }];
  assertArrayContains(fixture, ["deno"]);
  assertArrayContains(fixtureObject, [{ deno: "luv" }]);
  let didThrow;
  try {
    assertArrayContains(fixtureObject, [{ deno: "node" }]);
    didThrow = false;
  } catch (e) {
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assertEquals(didThrow, true);
});

test(function testingAssertStringContainsThrow(): void {
  let didThrow = false;
  try {
    assertStrContains("Denosaurus from Jurassic", "Raptor");
  } catch (e) {
    assert(
      e.message ===
        `actual: "Denosaurus from Jurassic" expected to contains: "Raptor"`
    );
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertStringMatching(): void {
  assertMatch("foobar@deno.com", RegExp(/[a-zA-Z]+@[a-zA-Z]+.com/));
});

test(function testingAssertStringMatchingThrows(): void {
  let didThrow = false;
  try {
    assertMatch("Denosaurus from Jurassic", RegExp(/Raptor/));
  } catch (e) {
    assert(
      e.message ===
        `actual: "Denosaurus from Jurassic" expected to match: "/Raptor/"`
    );
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertsUnimplemented(): void {
  let didThrow = false;
  try {
    unimplemented();
  } catch (e) {
    assert(e.message === "unimplemented");
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertsUnreachable(): void {
  let didThrow = false;
  try {
    unreachable();
  } catch (e) {
    assert(e.message === "unreachable");
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assert(didThrow);
});

test(function testingAssertFail(): void {
  assertThrows(fail, AssertionError, "Failed assertion.");
  assertThrows(
    (): void => {
      fail("foo");
    },
    AssertionError,
    "Failed assertion: foo"
  );
});

test(function testingAssertFailWithWrongErrorClass(): void {
  assertThrows(
    (): void => {
      //This next assertThrows will throw an AssertionError due to the wrong
      //expected error class
      assertThrows(
        (): void => {
          fail("foo");
        },
        Error,
        "Failed assertion: foo"
      );
    },
    AssertionError,
    `Expected error to be instance of "Error", but was "AssertionError"`
  );
});

const createHeader = (): string[] => [
  "",
  "",
  `    ${gray(bold("[Diff]"))} ${red(bold("Left"))} / ${green(bold("Right"))}`,
  "",
  "",
];

const added: (s: string) => string = (s: string): string => green(bold(s));
const removed: (s: string) => string = (s: string): string => red(bold(s));

test({
  name: "pass case",
  fn(): void {
    assertEquals({ a: 10 }, { a: 10 });
    assertEquals(true, true);
    assertEquals(10, 10);
    assertEquals("abc", "abc");
    assertEquals({ a: 10, b: { c: "1" } }, { a: 10, b: { c: "1" } });
  },
});

test({
  name: "failed with number",
  fn(): void {
    assertThrows(
      (): void => assertEquals(1, 2),
      AssertionError,
      [...createHeader(), removed(`-   1`), added(`+   2`), ""].join("\n")
    );
  },
});

test({
  name: "failed with number vs string",
  fn(): void {
    assertThrows(
      (): void => assertEquals(1, "1"),
      AssertionError,
      [...createHeader(), removed(`-   1`), added(`+   "1"`)].join("\n")
    );
  },
});

test({
  name: "failed with array",
  fn(): void {
    assertThrows(
      (): void => assertEquals([1, "2", 3], ["1", "2", 3]),
      AssertionError,
      [
        ...createHeader(),
        white("    Array ["),
        removed(`-     1,`),
        added(`+     "1",`),
        white('      "2",'),
        white("      3,"),
        white("    ]"),
        "",
      ].join("\n")
    );
  },
});

test({
  name: "failed with object",
  fn(): void {
    assertThrows(
      (): void => assertEquals({ a: 1, b: "2", c: 3 }, { a: 1, b: 2, c: [3] }),
      AssertionError,
      [
        ...createHeader(),
        white("    Object {"),
        white(`      "a": 1,`),
        added(`+     "b": 2,`),
        added(`+     "c": Array [`),
        added(`+       3,`),
        added(`+     ],`),
        removed(`-     "b": "2",`),
        removed(`-     "c": 3,`),
        white("    }"),
        "",
      ].join("\n")
    );
  },
});
