const { strictEqual } = require("assert");

Deno.test("Correct assertion", () => {
  strictEqual(20, 20);
});

Deno.test("Failed assertion", () => {
  strictEqual(10, 20);
});
