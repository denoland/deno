// Enable ops sanitizer at module level
Deno.test.sanitizer({ ops: true });

// This test overrides the module-level sanitizer, so the timer leak is allowed
Deno.test({
  name: "timer leak allowed by per-test override",
  sanitizeOps: false,
  fn() {
    setTimeout(() => {}, 10000);
  },
});
