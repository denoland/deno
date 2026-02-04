// Regression test for https://github.com/denoland/deno/issues/28629
// crypto.randomBytes should return a buffer with its own ArrayBuffer,
// not a shared pool buffer.

import { randomBytes } from "node:crypto";

// Test that the underlying ArrayBuffer has the expected size
const buf8 = randomBytes(8);
if (buf8.buffer.byteLength !== 8) {
  throw new Error(`Expected buffer.byteLength to be 8, got ${buf8.buffer.byteLength}`);
}

// Test that multiple calls return buffers with different underlying data
// This was broken when using shared pool allocation - all values were identical
const val1 = new BigUint64Array(randomBytes(8).buffer)[0];
const val2 = new BigUint64Array(randomBytes(8).buffer)[0];
const val3 = new BigUint64Array(randomBytes(8).buffer)[0];

// While extremely unlikely to be identical by chance, this specifically
// tests the fix for the bug where all values were the same due to shared buffer pool
if (val1 === val2 && val2 === val3) {
  throw new Error(`All random values were identical: ${val1} (shared buffer pool bug)`);
}

console.log("ok");
