Deno.test("standalone test", () => {
  if (1 + 1 !== 2) throw new Error("fail");
});
