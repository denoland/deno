// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertStackTraceEquals, test } from "checkin:testing";
import {
  asyncPromiseId,
  asyncYield,
  barrierAwait,
  barrierCreate,
} from "checkin:async";
import { asyncThrow } from "checkin:error";

// Test that stack traces from async ops are all sane
test(async function testAsyncThrow() {
  try {
    await asyncThrow("eager");
  } catch (e) {
    assertStackTraceEquals(
      e.stack,
      `TypeError: Error
        at asyncThrow (checkin:error:line:col)
        at testAsyncThrow (test:///unit/ops_async_test.ts:line:col)
      `,
    );
  }
  try {
    await asyncThrow("lazy");
  } catch (e) {
    // The stack may or may not include processTicksAndRejections depending
    // on whether the microtask runs inside op_run_microtasks (no ticks
    // scheduled) or processTicksAndRejections (ticks scheduled).
    assert(e.stack.includes("asyncThrow"));
    assert(e.stack.includes("testAsyncThrow"));
  }
  try {
    await asyncThrow("deferred");
  } catch (e) {
    assert(e.stack.includes("asyncThrow"));
    assert(e.stack.includes("testAsyncThrow"));
  }
});

test(async function testAsyncOp() {
  await asyncYield();
});

// Test a large number of async ops resolving at the same time. This stress-tests both
// large-batch op dispatch and the JS-side promise-tracking implementation.
test(async function testAsyncBarrier() {
  const count = 1e5;
  barrierCreate("barrier", count);
  const promises = [];
  for (let i = 0; i < count; i++) {
    promises.push(barrierAwait("barrier"));
  }
  await Promise.all(promises);
});

test(async function promiseId() {
  const id = await asyncPromiseId();

  assert(typeof id === "number");
  assert(id > 0);
});

// Promise ids are i32s that wrap back to 0 past 2^31 - 1. Test that an op
// that is still pending when the wraparound happens (and whose ring slot has
// been reclaimed in the meantime, moving it to the overflow map) resolves
// correctly. https://github.com/denoland/deno/issues/22759
test(async function testPromiseIdWraparound() {
  const distanceToWrap = 5000;
  Deno.core.__setNextPromiseId(2 ** 31 - distanceToWrap);

  barrierCreate("wraparound_barrier", 2);
  const pending = barrierAwait("wraparound_barrier");

  // Churn through enough ops that `pending` is evicted from the promise ring
  // (RING_SIZE == 4096 ids later) and the id counter wraps past 2^31 - 1.
  for (let i = 0; i < distanceToWrap + 1000; i++) {
    await asyncYield();
  }

  // The counter must have wrapped back to a small non-negative id.
  const id = await asyncPromiseId();
  assert(id >= 0);
  assert(id < distanceToWrap);

  // Releasing the barrier resolves `pending`, which by now lives in the
  // overflow map on the far side of the wraparound.
  await Promise.all([pending, barrierAwait("wraparound_barrier")]);
});
