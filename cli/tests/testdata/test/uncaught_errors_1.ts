Deno.test("foo 1", () => {
  throw new Error("foo 1 message");
});

Deno.test("foo 2", () => {});

Deno.test("foo 3", () => {
  Promise.reject(new Error("foo 3 message"));
});
