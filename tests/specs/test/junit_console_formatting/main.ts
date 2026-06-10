Deno.test("test", () => {
  throw new Error("Oh No \x1b[31mRed\x1b[0m Error");
});
