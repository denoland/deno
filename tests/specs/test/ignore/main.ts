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

if (Deno.test.skip !== Deno.test.ignore) {
  throw new Error("Deno.test.skip must alias Deno.test.ignore");
}
if (Deno.test.skip.each !== Deno.test.ignore.each) {
  throw new Error("Deno.test.skip.each must alias Deno.test.ignore.each");
}

const skip: Deno.TestIgnore = Deno.test.skip;
skip("test 10", () => {
  throw new Error("unreachable");
});
skip.each([[11], [12]])("test %i", () => {
  throw new Error("unreachable");
});
