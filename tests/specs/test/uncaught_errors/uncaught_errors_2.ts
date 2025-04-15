Deno.test("bar 1", () => {});

Deno.test("bar 2", () => {
  throw new Error("bar 2");
});
Deno.test("bar 3", () => {
  throw new Error("bar 3 message");
});
