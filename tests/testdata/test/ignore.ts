for (let i = 0; i < 5; i++) {
  Deno.test({
    name: `test ${i}`,
    ignore: true,
    fn() {
      throw new Error("unreachable");
    },
  });
}
for (let i = 5; i < 10; i++) {
  Deno.test.ignore({
    name: `test ${i}`,
    fn() {
      throw new Error("unreachable");
    },
  });
}
