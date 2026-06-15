// Regression test for https://github.com/denoland/deno/issues/22927.
// The timer leaks from a sanitizer-ignoring test, then fires while the next
// test is running. That disappearance must not be attributed to the next test.

Deno.test(
  { sanitizeOps: false, sanitizeResources: false },
  function test1() {
    setTimeout(() => {}, 1000);
  },
);

Deno.test(async function test2() {
  await new Promise((resolve) => setTimeout(resolve, 2000));
});
