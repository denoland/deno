Deno.test({
  name: "hello world test",
  fn(): void {
    const world = "world";
    if ("world" !== world) {
      throw new Error("world !== world");
    }
  },
});
