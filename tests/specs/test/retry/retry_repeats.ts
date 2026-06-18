// `retry` and `repeats` compose: each repetition may itself be retried.
// `repeats: 1` runs the test twice (two repetitions); `retry: 1` gives each
// repetition up to two attempts. The first attempt of every repetition fails
// and the retry passes, so both repetitions pass and the test is flaky.
let runs = 0;
Deno.test({
  name: "compose",
  repeats: 1,
  retry: 1,
  fn() {
    runs++;
    // Odd runs are the first attempt of a repetition (fail); even runs are the
    // retry (pass).
    if (runs % 2 === 1) {
      throw new Error("first attempt fails");
    }
  },
});
