"use strict";
const assert = require("node:assert");

// Test 1: require of ESM that throws a simple error can be caught
let caught = false;
try {
  require("./throw.mjs");
} catch (e) {
  caught = true;
  assert.strictEqual(e.message, "STOP");
} finally {
  assert.ok(caught, "require of throwing ESM should have been caught");
}

// Test 2: require of ESM that throws a global error can be caught
globalThis.err = new Error("top-level error");
assert.throws(() => require("./throw_global.mjs"), globalThis.err);

console.log("caught require of throwing esm");
