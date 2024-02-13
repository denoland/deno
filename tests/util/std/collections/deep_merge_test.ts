// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertStrictEquals } from "../assert/mod.ts";
import { deepMerge } from "./deep_merge.ts";

Deno.test("deepMerge: simple merge", () => {
  assertEquals(
    deepMerge({
      foo: true,
    }, {
      bar: true,
    }),
    {
      foo: true,
      bar: true,
    },
  );
});

Deno.test("deepMerge: symbol merge", () => {
  assertEquals(
    deepMerge({}, {
      [Symbol.for("deepmerge.test")]: true,
    }),
    {
      [Symbol.for("deepmerge.test")]: true,
    },
  );
});

Deno.test("deepMerge: ignore non enumerable", () => {
  assertEquals(
    deepMerge(
      {},
      Object.defineProperties({}, {
        foo: { enumerable: false, value: true },
        bar: { enumerable: true, value: true },
      }),
    ),
    {
      bar: true,
    },
  );
});

Deno.test("deepMerge: nested merge", () => {
  assertEquals(
    deepMerge({
      foo: {
        bar: true,
      },
    }, {
      foo: {
        baz: true,
        quux: {},
      },
      qux: true,
    }),
    {
      foo: {
        bar: true,
        baz: true,
        quux: {},
      },
      qux: true,
    },
  );
});

Deno.test("deepMerge: prevent prototype merge", () => {
  assertEquals(
    deepMerge({
      constructor: undefined,
    }, {
      foo: true,
    }),
    {
      constructor: undefined,
      foo: true,
    },
  );
});

Deno.test("deepMerge: prevent calling Object.prototype.__proto__ accessor property", () => {
  Object.defineProperty(Object.prototype, "__proto__", {
    get() {
      throw new Error(
        "Unexpected Object.prototype.__proto__ getter property call",
      );
    },
    set() {
      throw new Error(
        "Unexpected Object.prototype.__proto__ setter property call",
      );
    },
    configurable: true,
  });
  try {
    assertEquals<unknown>(
      deepMerge({
        foo: true,
      }, {
        bar: true,
        ["__proto__"]: {},
      }),
      {
        foo: true,
        bar: true,
      },
    );
  } finally {
    // deno-lint-ignore no-explicit-any
    delete (Object.prototype as any).__proto__;
  }
});

Deno.test("deepMerge: override target (non-mergeable source)", () => {
  assertEquals(
    deepMerge({
      foo: {
        bar: true,
      },
    }, {
      foo: true,
    }),
    {
      foo: true,
    },
  );
});

Deno.test("deepMerge: override target (non-mergeable destination, object like)", () => {
  const CustomClass = class {};
  assertEquals(
    deepMerge({
      foo: new CustomClass(),
    }, {
      foo: true,
    }),
    {
      foo: true,
    },
  );
});

Deno.test("deepMerge: override target (non-mergeable destination, array like)", () => {
  assertEquals(
    deepMerge({
      foo: [],
    }, {
      foo: true,
    }),
    {
      foo: true,
    },
  );
});

Deno.test("deepMerge: override target (different object like source and destination)", () => {
  assertEquals(
    deepMerge({
      foo: {},
    }, {
      foo: [1, 2],
    }),
    {
      foo: [1, 2],
    },
  );
  assertEquals(
    deepMerge({
      foo: [],
    }, {
      foo: { bar: true },
    }),
    {
      foo: { bar: true },
    },
  );
});

Deno.test("deepMerge: primitive types handling", () => {
  const CustomClass = class {};
  const expected = {
    boolean: true,
    null: null,
    undefined: undefined,
    number: 1,
    bigint: 1n,
    string: "string",
    symbol: Symbol.for("deepmerge.test"),
    object: { foo: true },
    regexp: /regex/,
    date: new Date(),
    function() {},
    async async() {},
    arrow: () => {},
    class: new CustomClass(),
    get get() {
      return true;
    },
  };
  assertEquals(
    deepMerge({
      boolean: false,
      null: undefined,
      undefined: null,
      number: -1,
      bigint: -1n,
      string: "foo",
      symbol: Symbol(),
      object: null,
      regexp: /foo/,
      date: new Date(0),
      function: function () {},
      async: async function () {},
      arrow: () => {},
      class: null,
      get: false,
    }, expected),
    expected,
  );
});

Deno.test("deepMerge: array merge (replace)", () => {
  assertEquals(
    deepMerge({
      foo: [1, 2, 3],
    }, {
      foo: [4, 5, 6],
    }, { arrays: "replace" }),
    {
      foo: [4, 5, 6],
    },
  );
});

Deno.test("deepMerge: array merge (merge)", () => {
  assertEquals(
    deepMerge({
      foo: [1, 2, 3],
    }, {
      foo: [4, 5, 6],
    }, { arrays: "merge" }),
    {
      foo: [1, 2, 3, 4, 5, 6],
    },
  );
});

Deno.test("deepMerge: maps merge (replace)", () => {
  assertEquals(
    deepMerge({
      map: new Map([["foo", true]]),
    }, {
      map: new Map([["bar", true]]),
    }, { maps: "replace" }),
    {
      map: new Map([["bar", true]]),
    },
  );
});

Deno.test("deepMerge: maps merge (merge)", () => {
  assertEquals(
    deepMerge({
      map: new Map([["foo", true]]),
    }, {
      map: new Map([["bar", true]]),
    }, { maps: "merge" }),
    {
      map: new Map([["foo", true], ["bar", true]]),
    },
  );
});

Deno.test("deepMerge: sets merge (replace)", () => {
  assertEquals(
    deepMerge({
      set: new Set(["foo"]),
    }, {
      set: new Set(["bar"]),
    }, { sets: "replace" }),
    {
      set: new Set(["bar"]),
    },
  );
});

Deno.test("deepMerge: sets merge (merge)", () => {
  assertEquals(
    deepMerge({
      set: new Set(["foo"]),
    }, {
      set: new Set(["bar"]),
    }, { sets: "merge" }),
    {
      set: new Set(["foo", "bar"]),
    },
  );
});

Deno.test("deepMerge: nested replace", () => {
  assertEquals(
    deepMerge(
      {
        a: "A1",
        b: ["B11", "B12"],
        c: {
          d: "D1",
          e: ["E11"],
        },
      },
      {
        b: [],
        c: {
          d: "D2",
          e: [],
        },
      },
      {
        arrays: "replace",
      },
    ),
    {
      a: "A1",
      b: [],
      c: {
        d: "D2",
        e: [],
      },
    },
  );
});

Deno.test("deepMerge: complex test", () => {
  assertEquals(
    deepMerge({
      foo: {
        bar: {
          quux: new Set(["foo"]),
          grault: {},
        },
      },
    }, {
      foo: {
        bar: {
          baz: true,
          qux: [1, 2],
          grault: {
            garply: false,
          },
        },
        corge: "deno",
        [Symbol.for("deepmerge.test")]: true,
      },
    }),
    {
      foo: {
        bar: {
          quux: new Set(["foo"]),
          baz: true,
          qux: [1, 2],
          grault: {
            garply: false,
          },
        },
        corge: "deno",
        [Symbol.for("deepmerge.test")]: true,
      },
    },
  );
});

Deno.test("deepMerge: handle circular references", () => {
  const expected = { foo: true } as { foo: boolean; bar: unknown };
  expected.bar = expected;
  assertEquals(deepMerge({}, expected), expected);
  assertEquals(deepMerge(expected, {}), expected);
  assertEquals(deepMerge(expected, expected), expected);

  const source = {
    foo: { b: { c: { d: {} } } },
    bar: {},
  };
  const object = {
    foo: { a: 1 },
    bar: { a: 2 },
  };

  source.foo.b.c.d = source;
  // deno-lint-ignore no-explicit-any
  (source.bar as any).b = source.foo.b;
  // deno-lint-ignore no-explicit-any
  const result: any = deepMerge(source, object);
  assertStrictEquals(result.foo.b.c.d, result.foo.b.c.d.foo.b.c.d);
});

Deno.test("deepMerge: target object is not modified", () => {
  const record = {
    foo: {
      bar: true,
    },
    baz: [1, 2, 3],
    quux: new Set([1, 2, 3]),
  };
  assertEquals(
    deepMerge(record, {
      foo: {
        qux: false,
      },
      baz: [4, 5, 6],
      quux: new Set([4, 5, 6]),
    }, { arrays: "merge", sets: "merge" }),
    {
      foo: {
        bar: true,
        qux: false,
      },
      baz: [1, 2, 3, 4, 5, 6],
      quux: new Set([1, 2, 3, 4, 5, 6]),
    },
  );
  assertEquals(record, {
    foo: {
      bar: true,
    },
    baz: [1, 2, 3],
    quux: new Set([1, 2, 3]),
  });
});
