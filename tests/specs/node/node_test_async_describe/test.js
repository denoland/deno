// deno-lint-ignore-file

// Regression test for https://github.com/denoland/deno/issues/36876
// Async `describe` bodies should properly register nested suites, tests, and hooks.

import assert from "node:assert";
import { describe, it } from "node:test";

describe("Async Test Suite", async () => {
  const i = await Promise.resolve(42);
  let assertionCount = 0;

  it("should handle async operations", async () => {
    const result = await Promise.resolve(i * 2);
    assertionCount++;
    assert.strictEqual(result, 84);
  });

  describe("Nested Async Suite", async () => {
    const j = await Promise.resolve(7);

    it("should handle nested async operations", async () => {
      const result = await Promise.resolve(i + j);
      assertionCount++;
      assert.strictEqual(result, 49);
    });

    describe("Even More Nested Async Suite", async () => {
      const k = await Promise.resolve(3);

      it("should handle deeply nested async operations", async () => {
        const result = await Promise.resolve(i * j * k);
        assertionCount++;
        assert.strictEqual(result, 882);
      });
    });
  });

  it("should have the correct number of assertions after async tests", () => {
    assert.strictEqual(assertionCount, 3);
  });
});
