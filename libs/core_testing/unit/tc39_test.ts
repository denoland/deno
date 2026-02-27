// Copyright 2018-2025 the Deno authors. MIT license.
import { assert, fail, test } from "checkin:testing";

// Verify that "array by copy" proposal is enabled (https://github.com/tc39/proposal-change-array-by-copy)
test(function testArrayByCopy() {
  const a = [1, 2, 3];
  const b = a.toReversed();
  if (!(a[0] === 1 && a[1] === 2 && a[2] === 3)) {
    fail("Expected a to be intact");
  }
  if (!(b[0] === 3 && b[1] === 2 && b[2] === 1)) {
    fail("Expected b to be reversed");
  }
});

// Verify that "Array.fromAsync" proposal is enabled (https://github.com/tc39/proposal-array-from-async)
test(async function testArrayFromAsync() {
  const b = await Array.fromAsync(new Map([[1, 2], [3, 4]]));
  if (b[0][0] !== 1 || b[0][1] !== 2 || b[1][0] !== 3 || b[1][1] !== 4) {
    fail("failed");
  }
});

// Verify that "Iterator helpers" proposal is enabled (https://github.com/tc39/proposal-iterator-helpers)
test(function testIteratorHelpers() {
  function* naturals() {
    let i = 0;
    while (true) {
      yield i;
      i += 1;
    }
  }

  // @ts-expect-error: Not available in TypeScript yet
  const a = naturals().take(5).toArray();
  if (a[0] !== 0 || a[1] !== 1 || a[2] !== 2 || a[3] !== 3 || a[4] !== 4) {
    fail("failed");
  }
});

// Verify that "Set methods" proposal is enabled (https://github.com/tc39/proposal-set-methods)
test(function testSetMethods() {
  const a: Set<number> = new Set([1, 2, 3]).intersection(new Set([3, 4, 5]));
  if (a.size !== 1 && !a.has(3)) {
    fail("failed");
  }
});

// Verify that the "Temporal" proposal is enabled (https://github.com/tc39/proposal-temporal)
test(function testTemporal() {
  // @ts-expect-error: Not available in TypeScript yet
  assert(typeof Temporal !== "undefined");
});

// Verify that the "Float16Array" proposal is enabled (https://github.com/tc39/proposal-float16array)
test(function testFloat16Array() {
  // @ts-expect-error: Not available in TypeScript yet
  const a = new Float16Array([Math.PI]);
  assert(a[0] === 3.140625);
  // @ts-expect-error: Not available in TypeScript yet
  assert(typeof DataView.prototype.getFloat16 !== "undefined");
  // @ts-expect-error: Not available in TypeScript yet
  assert(typeof DataView.prototype.setFloat16 !== "undefined");
});
