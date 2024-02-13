// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  AssertionError,
  assertObjectMatch,
  assertThrows,
} from "./mod.ts";

Deno.test("AssertObjectMatching", function () {
  const sym = Symbol("foo");
  const a = { foo: true, bar: false };
  const b = { ...a, baz: a };
  const c = { ...b, qux: b };
  const d = { corge: c, grault: c };
  const e = { foo: true } as { [key: string]: unknown };
  e.bar = e;
  const f = { [sym]: true, bar: false };
  interface r {
    foo: boolean;
    bar: boolean;
  }
  const g: r = { foo: true, bar: false };
  const h = { foo: [1, 2, 3], bar: true };
  const i = { foo: [a, e], bar: true };
  const j = { foo: [[1, 2, 3]], bar: true };
  const k = { foo: [[1, [2, [3]]]], bar: true };
  const l = { foo: [[1, [2, [a, e, j, k]]]], bar: true };
  const m = { foo: /abc+/i, bar: [/abc/g, /abc/m] };
  const n = {
    foo: new Set(["foo", "bar"]),
    bar: new Map([
      ["foo", 1],
      ["bar", 2],
    ]),
    baz: new Map([
      ["a", a],
      ["b", b],
    ]),
  };

  // Simple subset
  assertObjectMatch(a, {
    foo: true,
  });
  // Subset with another subset
  assertObjectMatch(b, {
    foo: true,
    baz: { bar: false },
  });
  // Subset with multiple subsets
  assertObjectMatch(c, {
    foo: true,
    baz: { bar: false },
    qux: {
      baz: { foo: true },
    },
  });
  // Subset with same object reference as subset
  assertObjectMatch(d, {
    corge: {
      foo: true,
      qux: { bar: false },
    },
    grault: {
      bar: false,
      qux: { foo: true },
    },
  });
  // Subset with circular reference
  assertObjectMatch(e, {
    foo: true,
    bar: {
      bar: {
        bar: {
          foo: true,
        },
      },
    },
  });
  // Subset with interface
  assertObjectMatch(g, { bar: false });
  // Subset with same symbol
  assertObjectMatch(f, {
    [sym]: true,
  });
  // Subset with array inside
  assertObjectMatch(h, { foo: [] });
  assertObjectMatch(h, { foo: [1, 2] });
  assertObjectMatch(h, { foo: [1, 2, 3] });
  assertObjectMatch(i, { foo: [{ bar: false }] });
  assertObjectMatch(i, {
    foo: [{ bar: false }, { bar: { bar: { bar: { foo: true } } } }],
  });
  // Subset with nested array inside
  assertObjectMatch(j, { foo: [[1, 2, 3]] });
  assertObjectMatch(k, { foo: [[1, [2, [3]]]] });
  assertObjectMatch(l, { foo: [[1, [2, [a, e, j, k]]]] });
  // Regexp
  assertObjectMatch(m, { foo: /abc+/i });
  assertObjectMatch(m, { bar: [/abc/g, /abc/m] });
  //Built-in data structures
  assertObjectMatch(n, { foo: new Set(["foo"]) });
  assertObjectMatch(n, { bar: new Map([["bar", 2]]) });
  assertObjectMatch(n, { baz: new Map([["b", b]]) });
  assertObjectMatch(n, { baz: new Map([["b", { foo: true }]]) });

  // Missing key
  {
    let didThrow;
    try {
      assertObjectMatch(
        {
          foo: true,
        },
        {
          foo: true,
          bar: false,
        },
      );
      didThrow = false;
    } catch (e) {
      assert(e instanceof AssertionError);
      didThrow = true;
    }
    assertEquals(didThrow, true);
  }
  // Simple subset
  {
    let didThrow;
    try {
      assertObjectMatch(a, {
        foo: false,
      });
      didThrow = false;
    } catch (e) {
      assert(e instanceof AssertionError);
      didThrow = true;
    }
    assertEquals(didThrow, true);
  }
  // Subset with another subset
  {
    let didThrow;
    try {
      assertObjectMatch(b, {
        foo: true,
        baz: { bar: true },
      });
      didThrow = false;
    } catch (e) {
      assert(e instanceof AssertionError);
      didThrow = true;
    }
    assertEquals(didThrow, true);
  }
  // Subset with multiple subsets
  {
    let didThrow;
    try {
      assertObjectMatch(c, {
        foo: true,
        baz: { bar: false },
        qux: {
          baz: { foo: false },
        },
      });
      didThrow = false;
    } catch (e) {
      assert(e instanceof AssertionError);
      didThrow = true;
    }
    assertEquals(didThrow, true);
  }
  // Subset with same object reference as subset
  {
    let didThrow;
    try {
      assertObjectMatch(d, {
        corge: {
          foo: true,
          qux: { bar: true },
        },
        grault: {
          bar: false,
          qux: { foo: false },
        },
      });
      didThrow = false;
    } catch (e) {
      assert(e instanceof AssertionError);
      didThrow = true;
    }
    assertEquals(didThrow, true);
  }
  // Subset with circular reference
  {
    let didThrow;
    try {
      assertObjectMatch(e, {
        foo: true,
        bar: {
          bar: {
            bar: {
              foo: false,
            },
          },
        },
      });
      didThrow = false;
    } catch (e) {
      assert(e instanceof AssertionError);
      didThrow = true;
    }
    assertEquals(didThrow, true);
  }
  // Subset with symbol key but with string key subset
  {
    let didThrow;
    try {
      assertObjectMatch(f, {
        foo: true,
      });
      didThrow = false;
    } catch (e) {
      assert(e instanceof AssertionError);
      didThrow = true;
    }
    assertEquals(didThrow, true);
  }
  // Subset with array inside but doesn't match key subset
  {
    let didThrow;
    try {
      assertObjectMatch(i, {
        foo: [1, 2, 3, 4],
      });
      didThrow = false;
    } catch (e) {
      assert(e instanceof AssertionError);
      didThrow = true;
    }
    assertEquals(didThrow, true);
  }
  {
    let didThrow;
    try {
      assertObjectMatch(i, {
        foo: [{ bar: true }, { foo: false }],
      });
      didThrow = false;
    } catch (e) {
      assert(e instanceof AssertionError);
      didThrow = true;
    }
    assertEquals(didThrow, true);
  }
  // actual/expected value as instance of class
  {
    class A {
      a: number;
      constructor(a: number) {
        this.a = a;
      }
    }
    assertObjectMatch({ test: new A(1) }, { test: { a: 1 } });
    assertObjectMatch({ test: { a: 1 } }, { test: { a: 1 } });
    assertObjectMatch({ test: { a: 1 } }, { test: new A(1) });
    assertObjectMatch({ test: new A(1) }, { test: new A(1) });
  }
  {
    // actual/expected contains same instance of Map/TypedArray/etc
    const body = new Uint8Array([0, 1, 2]);
    assertObjectMatch({ body, foo: "foo" }, { body });
  }
  {
    // match subsets of arrays
    assertObjectMatch(
      { positions: [[1, 2, 3, 4]] },
      {
        positions: [[1, 2, 3]],
      },
    );
  }
  //Regexp
  assertThrows(() => assertObjectMatch(m, { foo: /abc+/ }), AssertionError);
  assertThrows(() => assertObjectMatch(m, { foo: /abc*/i }), AssertionError);
  assertThrows(
    () => assertObjectMatch(m, { bar: [/abc/m, /abc/g] }),
    AssertionError,
  );
  //Built-in data structures
  assertThrows(
    () => assertObjectMatch(n, { foo: new Set(["baz"]) }),
    AssertionError,
  );
  assertThrows(
    () => assertObjectMatch(n, { bar: new Map([["bar", 3]]) }),
    AssertionError,
  );
  assertThrows(
    () => assertObjectMatch(n, { baz: new Map([["a", { baz: true }]]) }),
    AssertionError,
  );
  // null in the first argument throws an assertion error, rather than a TypeError: Invalid value used as weak map key
  assertThrows(
    () => assertObjectMatch({ foo: null }, { foo: { bar: 42 } }),
    AssertionError,
  );
  assertObjectMatch({ foo: null, bar: null }, { foo: null });
  assertObjectMatch({ foo: undefined, bar: null }, { foo: undefined });
  assertThrows(
    () => assertObjectMatch({ foo: undefined, bar: null }, { foo: null }),
    AssertionError,
  );
  // Non mapable primative types should throw a readable type error
  assertThrows(
    // @ts-expect-error Argument of type 'null' is not assignable to parameter of type 'Record<PropertyKey, any>'
    () => assertObjectMatch(null, { foo: 42 }),
    TypeError,
    "assertObjectMatch",
  );
  // @ts-expect-error Argument of type 'null' is not assignable to parameter of type 'Record<PropertyKey, any>'
  assertThrows(() => assertObjectMatch(null, { foo: 42 }), TypeError, "null"); // since typeof null is "object", want to make sure user knows the bad value is "null"
  assertThrows(
    // @ts-expect-error Argument of type 'undefined' is not assignable to parameter of type 'Record<PropertyKey, any>'
    () => assertObjectMatch(undefined, { foo: 42 }),
    TypeError,
    "assertObjectMatch",
  );
  // @ts-expect-error Argument of type 'number' is not assignable to parameter of type 'Record<PropertyKey, any>'
  assertThrows(() => assertObjectMatch(21, 42), TypeError, "assertObjectMatch");
  assertThrows(
    // @ts-expect-error Argument of type 'string' is not assignable to parameter of type 'Record<PropertyKey, any>'
    () => assertObjectMatch("string", "string"),
    TypeError,
    "assertObjectMatch",
  );
});
