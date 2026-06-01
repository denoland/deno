Deno.test("timer leak", () => {
  setTimeout(() => {}, 10000);
});
