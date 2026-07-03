// If any repetition fails, the whole test fails.
let runs = 0;
Deno.test({
  name: "repeated with failure",
  repeats: 3,
  fn() {
    runs++;
    if (runs === 2) {
      throw new Error("second run fails");
    }
  },
});
