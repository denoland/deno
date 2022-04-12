for (let i = 0; i < 10; i++) {
  Deno.bench({
    name: `bench${i}`,
    ignore: true,
    fn() {
      throw new Error("unreachable");
    },
  });
}
