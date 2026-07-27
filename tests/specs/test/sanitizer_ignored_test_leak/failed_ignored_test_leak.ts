// Even when another failure takes precedence over sanitizer reporting, leaks
// from a sanitizer-ignoring test must not be attributed to later tests.

Deno.test(
  { sanitizeOps: false, sanitizeResources: false },
  function ignoresSanitizersLeaksAndFails() {
    setTimeout(() => {}, 10);
    throw new Error("primary failure");
  },
);

Deno.test(async function laterTestWaitsForIgnoredTimer() {
  await new Promise((resolve) => setTimeout(resolve, 50));
});
