// No config file enables sanitizers, but module-level does
Deno.test.sanitizer({ ops: true });

Deno.test("timer leak caught by module sanitizer overriding config", () => {
  setTimeout(() => {}, 10000);
});
