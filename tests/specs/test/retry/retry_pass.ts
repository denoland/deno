// A flaky test that fails twice and then passes is reported as passed and
// counted as flaky.
let attempts = 0;
Deno.test({
  name: "flaky",
  retry: 3,
  fn() {
    attempts++;
    if (attempts < 3) {
      throw new Error("not yet");
    }
  },
});
