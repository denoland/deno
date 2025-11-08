// test_deepStrictEqual.mjs
import assert from "node:assert/strict";

try {
  assert.deepStrictEqual(new Number(1), new Number(2));
  console.log("❌ FAIL: Should have thrown AssertionError");
} catch (e) {
  if (e.name === "AssertionError") {
    console.log("✅ PASS: Correctly threw AssertionError");
  } else {
    console.log("❌ FAIL: Threw wrong error type:", e.name);
  }
}

// Should pass - same values
try {
  assert.deepStrictEqual(new Number(1), new Number(1));
  console.log("✅ PASS: Same Number objects are equal");
} catch (e) {
  console.log("❌ FAIL: Same Number objects should be equal");
}
