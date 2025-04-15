Deno.test("foo", () => {
  throw undefined;
});

Deno.test("bar", () => {
  throw null;
});

Deno.test("baz", () => {
  throw 123;
});

Deno.test("qux", () => {
  throw "Hello, world!";
});

Deno.test("quux", () => {
  throw [1, 2, 3];
});

Deno.test("quuz", () => {
  throw { a: "Hello, world!", b: [1, 2, 3] };
});
