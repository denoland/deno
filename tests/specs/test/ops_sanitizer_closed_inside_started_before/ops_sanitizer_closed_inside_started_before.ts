const timer = setTimeout(() => {}, 10000000000);

Deno.test("test 1", () => {
  clearTimeout(timer);
});
