// A test with `repeats` runs 1 + repeats times; all repetitions pass here.
let runs = 0;
Deno.test({
  name: "repeated",
  repeats: 2,
  fn() {
    runs++;
  },
});
