Deno.test("foo", () => {
  reportError(new Error("foo"));
  console.log(1);
});

Deno.test("bar", () => {});
