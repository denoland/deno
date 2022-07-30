Deno.test("foo 1", () => {
  throw new Error("foo 1 message");
});

Deno.test("foo 2", () => {});
