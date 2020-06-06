// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertNotEquals,
  assertStringContains,
  assertArrayContains,
  assertMatch,
  assertEquals,
  assertStrictEquals,
  assertThrows,
  assertThrowsAsync,
  AssertionError,
  equal,
  fail,
  unimplemented,
  unreachable,
} from "./asserts.ts";
import { red, green, gray, bold, yellow } from "../fmt/colors.ts";
const { test } = Deno;

test("testingEqual", function (): void {
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

test("testingNotEquals", function (): void {
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

test("testingAssertStringContains", function (): void {
  assertStringContains("Denosaurus", "saur");
  assertStringContains("Denosaurus", "Deno");
  assertStringContains("Denosaurus", "rus");
  let didThrow;
  try {
    assertStringContains("Denosaurus", "Raptor");
    didThrow = false;
  } catch (e) {
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assertEquals(didThrow, true);
});

test("testingArrayContains", function (): void {
  const fixture = ["deno", "iz", "luv"];
  const fixtureObject = [{ deno: "luv" }, { deno: "Js" }];
  assertArrayContains(fixture, ["deno"]);
  assertArrayContains(fixtureObject, [{ deno: "luv" }]);
  assertThrows(
    (): void => assertArrayContains(fixtureObject, [{ deno: "node" }]),
    AssertionError,
    `actual: "[ { deno: "luv" }, { deno: "Js" } ]" expected to contain: "[ { deno: "node" } ]"\nmissing: [ { deno: "node" } ]`
  );
});

test("testingAssertStringContainsThrow", function (): void {
  let didThrow = false;
  try {
    assertStringContains("Denosaurus from Jurassic", "Raptor");
  } catch (e) {
    assert(
      e.message ===
        `actual: "Denosaurus from Jurassic" expected to contain: "Raptor"`
    );
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assert(didThrow);
});

test("testingAssertStringMatching", function (): void {
  assertMatch("foobar@deno.com", RegExp(/[a-zA-Z]+@[a-zA-Z]+.com/));
});

test("testingAssertStringMatchingThrows", function (): void {
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

test("testingAssertsUnimplemented", function (): void {
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

test("testingAssertsUnreachable", function (): void {
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

test("testingAssertFail", function (): void {
  assertThrows(fail, AssertionError, "Failed assertion.");
  assertThrows(
    (): void => {
      fail("foo");
    },
    AssertionError,
    "Failed assertion: foo"
  );
});

test("testingAssertFailWithWrongErrorClass", function (): void {
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

test("testingAssertThrowsWithReturnType", () => {
  assertThrows(() => {
    throw new Error();
    return "a string";
  });
});

test("testingAssertThrowsAsyncWithReturnType", () => {
  assertThrowsAsync(() => {
    throw new Error();
    return Promise.resolve("a Promise<string>");
  });
});

const createHeader = (): string[] => [
  "",
  "",
  `    ${gray(bold("[Diff]"))} ${red(bold("Actual"))} / ${green(
    bold("Expected")
  )}`,
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
      [
        "Values are not equal:",
        ...createHeader(),
        removed(`-   ${yellow("1")}`),
        added(`+   ${yellow("2")}`),
        "",
      ].join("\n")
    );
  },
});

test({
  name: "failed with number vs string",
  fn(): void {
    assertThrows(
      (): void => assertEquals(1, "1"),
      AssertionError,
      [
        "Values are not equal:",
        ...createHeader(),
        removed(`-   ${yellow("1")}`),
        added(`+   "1"`),
      ].join("\n")
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
        "Values are not equal:",
        ...createHeader(),
        removed(`-   [ ${yellow("1")}, ${green('"2"')}, ${yellow("3")} ]`),
        added(`+   [ ${green('"1"')}, ${green('"2"')}, ${yellow("3")} ]`),
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
        "Values are not equal:",
        ...createHeader(),
        removed(
          `-   { a: ${yellow("1")}, b: ${green('"2"')}, c: ${yellow("3")} }`
        ),
        added(
          `+   { a: ${yellow("1")}, b: ${yellow("2")}, c: [ ${yellow("3")} ] }`
        ),
        "",
      ].join("\n")
    );
  },
});

test({
  name: "strict pass case",
  fn(): void {
    assertStrictEquals(true, true);
    assertStrictEquals(10, 10);
    assertStrictEquals("abc", "abc");

    const xs = [1, false, "foo"];
    const ys = xs;
    assertStrictEquals(xs, ys);

    const x = { a: 1 };
    const y = x;
    assertStrictEquals(x, y);
  },
});

test({
  name: "strict failed with structure diff",
  fn(): void {
    assertThrows(
      (): void => assertStrictEquals({ a: 1, b: 2 }, { a: 1, c: [3] }),
      AssertionError,
      [
        "Values are not strictly equal:",
        ...createHeader(),
        removed(`-   { a: ${yellow("1")}, b: ${yellow("2")} }`),
        added(`+   { a: ${yellow("1")}, c: [ ${yellow("3")} ] }`),
        "",
      ].join("\n")
    );
  },
});

test({
  name: "strict failed with reference diff",
  fn(): void {
    assertThrows(
      (): void => assertStrictEquals({ a: 1, b: 2 }, { a: 1, b: 2 }),
      AssertionError,
      [
        "Values have the same structure but are not reference-equal:\n",
        red(`     { a: ${yellow("1")}, b: ${yellow("2")} }`),
      ].join("\n")
    );
  },
});
