// CLI passes --sanitize-ops but module-level disables it
Deno.test.sanitizer({ ops: false });

Deno.test("timer leak allowed because module disables sanitizer", () => {
  setTimeout(() => {}, 10000);
});
