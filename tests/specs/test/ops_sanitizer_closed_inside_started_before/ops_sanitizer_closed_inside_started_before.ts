const timer = setTimeout(() => {}, 1000000);

Deno.test("test 1", () => {
  clearTimeout(timer);
});
