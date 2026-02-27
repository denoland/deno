"use strict";
const assert = require("node:assert");

// Test 1: require of ESM that throws a simple error can be caught
try {
  require("./throw.mjs");
  assert.fail("should have thrown");
} catch (e) {
  assert.strictEqual(e.message, "STOP");
}

// Test 2: require of ESM that throws a global error can be caught
globalThis.err = new Error("top-level error");
assert.throws(() => require("./throw_global.mjs"), globalThis.err);

console.log("caught require of throwing esm");
