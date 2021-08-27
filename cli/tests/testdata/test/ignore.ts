for (let i = 0; i < 10; i++) {
  Deno.test({
    name: `test ${i}`,
    ignore: true,
    fn() {
      throw new Error("unreachable");
    },
  });
}
