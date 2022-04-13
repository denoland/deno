Deno.test("hello", () => {
  throw new Error("boom!");
});


Deno.test("boom", () => {
  throw new Error("boom!");
});
