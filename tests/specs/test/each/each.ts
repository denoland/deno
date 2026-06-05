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
