Deno.test.sanitizer({ ops: true });

Deno.test("timer leak caught by module sanitizer", () => {
  setTimeout(() => {}, 10000);
});
