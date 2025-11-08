// test_partialDeepStrictEqual.mjs
import assert from 'node:assert';

// Test method exists
if (typeof assert.partialDeepStrictEqual === 'function') {
  console.log('✅ PASS: partialDeepStrictEqual method exists');
} else {
  console.log('❌ FAIL: partialDeepStrictEqual method missing');
}

// Test array subset functionality
try {
  assert.partialDeepStrictEqual([1, 2, 3, 4, 5], [2, 4]);
  console.log('✅ PASS: Array subset check works');
} catch (e) {
  console.log('❌ FAIL: Array subset check failed:', e.message);
}

// Test object subset functionality
try {
  assert.partialDeepStrictEqual({a: 1, b: 2, c: 3}, {a: 1, c: 3});
  console.log('✅ PASS: Object subset check works');
} catch (e) {
  console.log('❌ FAIL: Object subset check failed:', e.message);
}

// Test should fail case
try {
  assert.partialDeepStrictEqual([1, 2, 3], [4, 5]);
  console.log('❌ FAIL: Should have thrown for non-subset');
} catch (e) {
  console.log('✅ PASS: Correctly threw for non-subset');
}