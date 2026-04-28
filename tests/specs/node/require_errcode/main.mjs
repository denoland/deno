// Test that Module.prototype.require throws proper Node.js error codes
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);

// Test 1: non-string arg throws ERR_INVALID_ARG_TYPE
try {
  require(123);
  throw new Error("should have thrown");
} catch (e) {
  if (e.code !== "ERR_INVALID_ARG_TYPE") throw new Error(`expected ERR_INVALID_ARG_TYPE, got ${e.code}`);
  if (!(e instanceof TypeError)) throw new Error(`expected TypeError, got ${e.constructor.name}`);
  console.log("Test 1 passed: non-string arg gives ERR_INVALID_ARG_TYPE");
}

// Test 2: empty string throws ERR_INVALID_ARG_VALUE
try {
  require("");
  throw new Error("should have thrown");
} catch (e) {
  if (e.code !== "ERR_INVALID_ARG_VALUE") throw new Error(`expected ERR_INVALID_ARG_VALUE, got ${e.code}`);
  if (!(e instanceof TypeError)) throw new Error(`expected TypeError, got ${e.constructor.name}`);
  console.log("Test 2 passed: empty string gives ERR_INVALID_ARG_VALUE");
}

console.log("All require error code tests passed");
