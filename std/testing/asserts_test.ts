// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  _format,
  assert,
  assertNotEquals,
  assertStringContains,
  assertArrayContains,
  assertMatch,
  assertEquals,
  assertStrictEquals,
  assertNotStrictEquals,
  assertThrows,
  assertThrowsAsync,
  AssertionError,
  equal,
  fail,
  unimplemented,
  unreachable,
} from "./asserts.ts";
import { red, green, gray, bold, yellow, stripColor } from "../fmt/colors.ts";

Deno.test("testingEqual", function (): void {
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
      { hello: "world", hi: { there: "everyone" } },
    ),
  );
  assert(
    !equal(
      { hello: "world", hi: { there: "everyone" } },
      { hello: "world", hi: { there: "everyone else" } },
    ),
  );
  assert(equal(/deno/, /deno/));
  assert(!equal(/deno/, /node/));
  assert(equal(new Date(2019, 0, 3), new Date(2019, 0, 3)));
  assert(!equal(new Date(2019, 0, 3), new Date(2019, 1, 3)));
  assert(
    !equal(
      new Date(2019, 0, 3, 4, 20, 1, 10),
      new Date(2019, 0, 3, 4, 20, 1, 20),
    ),
  );
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
      ]),
    ),
  );
  assert(
    equal(
      new Map([["foo", new Map([["bar", "baz"]])]]),
      new Map([["foo", new Map([["bar", "baz"]])]]),
    ),
  );
  assert(
    equal(
      new Map([["foo", { bar: "baz" }]]),
      new Map([["foo", { bar: "baz" }]]),
    ),
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
      ]),
    ),
  );
  assert(equal(new Map([["foo", ["bar"]]]), new Map([["foo", ["bar"]]])));
  assert(!equal(new Map([["foo", "bar"]]), new Map([["bar", "baz"]])));
  assert(
    !equal(
      new Map([["foo", "bar"]]),
      new Map([
        ["foo", "bar"],
        ["bar", "baz"],
      ]),
    ),
  );
  assert(
    !equal(
      new Map([["foo", new Map([["bar", "baz"]])]]),
      new Map([["foo", new Map([["bar", "qux"]])]]),
    ),
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
  assert(
    equal(new URL("https://example.test"), new URL("https://example.test")),
  );
  assert(
    !equal(
      new URL("https://example.test"),
      new URL("https://example.test/with-path"),
    ),
  );
});

Deno.test("testingNotEquals", function (): void {
  const a = { foo: "bar" };
  const b = { bar: "foo" };
  assertNotEquals(a, b);
  assertNotEquals("Denosaurus", "Tyrannosaurus");
  assertNotEquals(
    new Date(2019, 0, 3, 4, 20, 1, 10),
    new Date(2019, 0, 3, 4, 20, 1, 20),
  );
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

Deno.test("testingAssertStringContains", function (): void {
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

Deno.test("testingArrayContains", function (): void {
  const fixture = ["deno", "iz", "luv"];
  const fixtureObject = [{ deno: "luv" }, { deno: "Js" }];
  assertArrayContains(fixture, ["deno"]);
  assertArrayContains(fixtureObject, [{ deno: "luv" }]);
  assertArrayContains(
    Uint8Array.from([1, 2, 3, 4]),
    Uint8Array.from([1, 2, 3]),
  );
  assertThrows(
    (): void => assertArrayContains(fixtureObject, [{ deno: "node" }]),
    AssertionError,
    `actual: "[
  {
    deno: "luv",
  },
  {
    deno: "Js",
  },
]" expected to contain: "[
  {
    deno: "node",
  },
]"
missing: [
  {
    deno: "node",
  },
]`,
  );
});

Deno.test("testingAssertStringContainsThrow", function (): void {
  let didThrow = false;
  try {
    assertStringContains("Denosaurus from Jurassic", "Raptor");
  } catch (e) {
    assert(
      e.message ===
        `actual: "Denosaurus from Jurassic" expected to contain: "Raptor"`,
    );
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assert(didThrow);
});

Deno.test("testingAssertStringMatching", function (): void {
  assertMatch("foobar@deno.com", RegExp(/[a-zA-Z]+@[a-zA-Z]+.com/));
});

Deno.test("testingAssertStringMatchingThrows", function (): void {
  let didThrow = false;
  try {
    assertMatch("Denosaurus from Jurassic", RegExp(/Raptor/));
  } catch (e) {
    assert(
      e.message ===
        `actual: "Denosaurus from Jurassic" expected to match: "/Raptor/"`,
    );
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assert(didThrow);
});

Deno.test("testingAssertsUnimplemented", function (): void {
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

Deno.test("testingAssertsUnreachable", function (): void {
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

Deno.test("testingAssertFail", function (): void {
  assertThrows(fail, AssertionError, "Failed assertion.");
  assertThrows(
    (): void => {
      fail("foo");
    },
    AssertionError,
    "Failed assertion: foo",
  );
});

Deno.test("testingAssertFailWithWrongErrorClass", function (): void {
  assertThrows(
    (): void => {
      //This next assertThrows will throw an AssertionError due to the wrong
      //expected error class
      assertThrows(
        (): void => {
          fail("foo");
        },
        TypeError,
        "Failed assertion: foo",
      );
    },
    AssertionError,
    `Expected error to be instance of "TypeError", but was "AssertionError"`,
  );
});

Deno.test("testingAssertThrowsWithReturnType", () => {
  assertThrows(() => {
    throw new Error();
  });
});

Deno.test("testingAssertThrowsAsyncWithReturnType", () => {
  assertThrowsAsync(() => {
    throw new Error();
  });
});

const createHeader = (): string[] => [
  "",
  "",
  `    ${gray(bold("[Diff]"))} ${red(bold("Actual"))} / ${
    green(bold("Expected"))
  }`,
  "",
  "",
];

const added: (s: string) => string = (s: string): string => green(bold(s));
const removed: (s: string) => string = (s: string): string => red(bold(s));

Deno.test({
  name: "pass case",
  fn(): void {
    assertEquals({ a: 10 }, { a: 10 });
    assertEquals(true, true);
    assertEquals(10, 10);
    assertEquals("abc", "abc");
    assertEquals({ a: 10, b: { c: "1" } }, { a: 10, b: { c: "1" } });
  },
});

Deno.test({
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
      ].join("\n"),
    );
  },
});

Deno.test({
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
      ].join("\n"),
    );
  },
});

Deno.test({
  name: "failed with array",
  fn(): void {
    assertThrows(
      (): void => assertEquals([1, "2", 3], ["1", "2", 3]),
      AssertionError,
      `
    [
-     1,
+     "1",
      "2",
      3,
    ]`,
    );
  },
});

Deno.test({
  name: "failed with object",
  fn(): void {
    assertThrows(
      (): void => assertEquals({ a: 1, b: "2", c: 3 }, { a: 1, b: 2, c: [3] }),
      AssertionError,
      `
    {
      a: 1,
+     b: 2,
+     c: [
+       3,
+     ],
-     b: "2",
-     c: 3,
    }`,
    );
  },
});

Deno.test({
  name: "failed with date",
  fn(): void {
    assertThrows(
      (): void =>
        assertEquals(
          new Date(2019, 0, 3, 4, 20, 1, 10),
          new Date(2019, 0, 3, 4, 20, 1, 20),
        ),
      AssertionError,
      [
        "Values are not equal:",
        ...createHeader(),
        removed(`-   ${new Date(2019, 0, 3, 4, 20, 1, 10).toISOString()}`),
        added(`+   ${new Date(2019, 0, 3, 4, 20, 1, 20).toISOString()}`),
        "",
      ].join("\n"),
    );
  },
});

Deno.test({
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

Deno.test({
  name: "strict failed with structure diff",
  fn(): void {
    assertThrows(
      (): void => assertStrictEquals({ a: 1, b: 2 }, { a: 1, c: [3] }),
      AssertionError,
      `
    {
      a: 1,
+     c: [
+       3,
+     ],
-     b: 2,
    }`,
    );
  },
});

Deno.test({
  name: "strict failed with reference diff",
  fn(): void {
    assertThrows(
      (): void => assertStrictEquals({ a: 1, b: 2 }, { a: 1, b: 2 }),
      AssertionError,
      `Values have the same structure but are not reference-equal:

    {
      a: 1,
      b: 2,
    }`,
    );
  },
});

Deno.test({
  name: "strictly unequal pass case",
  fn(): void {
    assertNotStrictEquals(true, false);
    assertNotStrictEquals(10, 11);
    assertNotStrictEquals("abc", "xyz");
    assertNotStrictEquals(1, "1");

    const xs = [1, false, "foo"];
    const ys = [1, true, "bar"];
    assertNotStrictEquals(xs, ys);

    const x = { a: 1 };
    const y = { a: 2 };
    assertNotStrictEquals(x, y);
  },
});

Deno.test({
  name: "strictly unequal fail case",
  fn(): void {
    assertThrows(
      () => assertNotStrictEquals(1, 1),
      AssertionError,
    );
  },
});

Deno.test({
  name: "assert* functions with specified type parameter",
  fn(): void {
    assertEquals<string>("hello", "hello");
    assertNotEquals<number>(1, 2);
    assertArrayContains<boolean>([true, false], [true]);
    const value = { x: 1 };
    assertStrictEquals<typeof value>(value, value);
    assertNotStrictEquals<object>(value, { x: 1 });
  },
});

Deno.test("Assert Throws Non-Error Fail", () => {
  assertThrows(
    () => {
      assertThrows(
        () => {
          throw "Panic!";
        },
        String,
        "Panic!",
      );
    },
    AssertionError,
    "A non-Error object was thrown.",
  );

  assertThrows(
    () => {
      assertThrows(() => {
        throw null;
      });
    },
    AssertionError,
    "A non-Error object was thrown.",
  );

  assertThrows(
    () => {
      assertThrows(() => {
        throw undefined;
      });
    },
    AssertionError,
    "A non-Error object was thrown.",
  );
});

Deno.test("Assert Throws Async Non-Error Fail", () => {
  assertThrowsAsync(
    () => {
      return assertThrowsAsync(
        () => {
          return Promise.reject("Panic!");
        },
        String,
        "Panic!",
      );
    },
    AssertionError,
    "A non-Error object was thrown or rejected.",
  );

  assertThrowsAsync(
    () => {
      return assertThrowsAsync(() => {
        return Promise.reject(null);
      });
    },
    AssertionError,
    "A non-Error object was thrown or rejected.",
  );

  assertThrowsAsync(
    () => {
      return assertThrowsAsync(() => {
        return Promise.reject(undefined);
      });
    },
    AssertionError,
    "A non-Error object was thrown or rejected.",
  );

  assertThrowsAsync(
    () => {
      return assertThrowsAsync(() => {
        throw undefined;
      });
    },
    AssertionError,
    "A non-Error object was thrown or rejected.",
  );
});

Deno.test("assertEquals diff for differently ordered objects", () => {
  assertThrows(
    () => {
      assertEquals(
        {
          aaaaaaaaaaaaaaaaaaaaaaaa: 0,
          bbbbbbbbbbbbbbbbbbbbbbbb: 0,
          ccccccccccccccccccccccc: 0,
        },
        {
          ccccccccccccccccccccccc: 1,
          aaaaaaaaaaaaaaaaaaaaaaaa: 0,
          bbbbbbbbbbbbbbbbbbbbbbbb: 0,
        },
      );
    },
    AssertionError,
    `
    {
      aaaaaaaaaaaaaaaaaaaaaaaa: 0,
      bbbbbbbbbbbbbbbbbbbbbbbb: 0,
-     ccccccccccccccccccccccc: 0,
+     ccccccccccccccccccccccc: 1,
    }`,
  );
});

// Check that the diff formatter overrides some default behaviours of
// `Deno.inspect()` which are problematic for diffing.
Deno.test("assert diff formatting", () => {
  // Wraps objects into multiple lines even when they are small. Prints trailing
  // commas.
  assertEquals(
    stripColor(_format({ a: 1, b: 2 })),
    `{
  a: 1,
  b: 2,
}`,
  );

  // Same for nested small objects.
  assertEquals(
    stripColor(_format([{ x: { a: 1, b: 2 }, y: ["a", "b"] }])),
    `[
  {
    x: {
      a: 1,
      b: 2,
    },
    y: [
      "a",
      "b",
    ],
  },
]`,
  );

  // Grouping is disabled.
  assertEquals(
    stripColor(_format(["i", "i", "i", "i", "i", "i", "i"])),
    `[
  "i",
  "i",
  "i",
  "i",
  "i",
  "i",
  "i",
]`,
  );
});

Deno.test("Assert Throws Parent Error", () => {
  assertThrows(
    () => {
      throw new AssertionError("Fail!");
    },
    Error,
    "Fail!",
  );
});

Deno.test("Assert Throws Async Parent Error", () => {
  assertThrowsAsync(
    () => {
      throw new AssertionError("Fail!");
    },
    Error,
    "Fail!",
  );
});
