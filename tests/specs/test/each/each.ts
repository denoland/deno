import { assertEquals } from "jsr:@std/assert";

// Array cases are spread as positional arguments. `%i`/`%s` consume them in
// order, `%#` is the zero-based case index.
Deno.test.each([
  [1, 1, 2],
  [1, 2, 3],
  [2, 1, 3],
])("add(%i, %i) = %i", (a, b, expected) => {
  assertEquals(a + b, expected);
});

// Object cases are passed as a single argument and interpolated via `$key`.
Deno.test.each([
  { a: 1, b: 1, sum: 2 },
  { a: 2, b: 3, sum: 5 },
])("$a + $b = $sum", ({ a, b, sum }) => {
  assertEquals(a + b, sum);
});

// Nested `$key.path` access and the case index.
Deno.test.each([
  { input: { value: "x" } },
  { input: { value: "y" } },
])("case #%# has $input.value", ({ input }) => {
  assertEquals(typeof input.value, "string");
});

// Primitive cases passed as a single argument, plus the TestContext.
Deno.test.each(["alpha", "beta"])("name is %s", (name, t) => {
  assertEquals(typeof name, "string");
  assertEquals(t.name, `name is ${name}`);
});

// Per-group options are supported via the optional second argument.
Deno.test.each([["a"], ["b"]])(
  "with options %s",
  { sanitizeOps: false, sanitizeResources: false },
  (s) => {
    assertEquals(typeof s, "string");
  },
);

// `%d`/`%i` truncate toward zero, `%f` keeps the decimal part, and `%%` is a
// literal percent sign. Each token consumes one positional value in order.
Deno.test.each([[3.9, 7.2, 2.5]])(
  "trunc %d, int %i, float %f, pct %%",
  (a, b, c) => {
    assertEquals(a + b + c, 13.6);
  },
);

// `%j` JSON-encodes the case value (object cases are passed as one argument).
Deno.test.each([{ k: 1 }])("json %j", (obj) => {
  assertEquals(obj.k, 1);
});

// `%o`/`%O` also JSON-encode, consuming positional values in order.
Deno.test.each([[{ k: 2 }, { m: 3 }]])("object %o and %O", (x, y) => {
  assertEquals(x.k + y.m, 5);
});
