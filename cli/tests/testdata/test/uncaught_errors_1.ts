Deno.test("foo 1 (renamed)", () => {
  throw new Error("foo 1 message");
});

Deno.test("foo 2 (renamed)", () => {});

Deno.test("foo 3 (renamed)", () => {
  Promise.reject(new Error("foo 3 message"));
});
