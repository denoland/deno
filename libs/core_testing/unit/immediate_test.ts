// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertArrayEquals, assertEquals, test } from "checkin:testing";

test(async function testImmediateBasic() {
  const { promise, resolve } = Promise.withResolvers();
  let called = false;
  setImmediate(() => {
    called = true;
    resolve(null);
  });
  assert(!called, "immediate should not fire synchronously");
  await promise;
  assert(called, "immediate should have fired");
});

test(async function testImmediateOrder() {
  const { promise, resolve } = Promise.withResolvers();
  const order: number[] = [];
  setImmediate(() => order.push(1));
  setImmediate(() => order.push(2));
  setImmediate(() => {
    order.push(3);
    resolve(null);
  });
  await promise;
  assertArrayEquals(order, [1, 2, 3]);
});

test(async function testClearImmediate() {
  const { promise, resolve } = Promise.withResolvers();
  let cleared = false;
  const imm = setImmediate(() => {
    cleared = true;
  });
  clearImmediate(imm);
  // Schedule another immediate to verify the cleared one didn't fire
  setImmediate(() => {
    resolve(null);
  });
  await promise;
  assert(!cleared, "cleared immediate should not have fired");
});

test(async function testImmediateFiresAfterTimer() {
  // An immediate queued from within a setTimeout callback should fire
  // in the next check phase, not in the same tick as the timer.
  const { promise, resolve } = Promise.withResolvers();
  const order: string[] = [];
  setTimeout(() => {
    order.push("timer");
    setImmediate(() => {
      order.push("immediate");
      resolve(null);
    });
  }, 1);
  await promise;
  assertArrayEquals(order, ["timer", "immediate"]);
});

test(async function testImmediateBeforeTimer() {
  // An already-queued immediate should fire before a timer with delay > 0.
  const { promise, resolve } = Promise.withResolvers();
  const order: string[] = [];
  let done = 0;
  function checkDone() {
    if (++done === 2) resolve(null);
  }
  // Queue the immediate first
  setImmediate(() => {
    order.push("immediate");
    checkDone();
  });
  // Then a timer with small delay
  setTimeout(() => {
    order.push("timer");
    checkDone();
  }, 50);
  await promise;
  assertArrayEquals(order, ["immediate", "timer"]);
});

test(async function testImmediateQueuedFromImmediate() {
  // Immediates queued from within an immediate callback fire in the
  // *next* event loop iteration's check phase (not the current one),
  // matching Node.js behavior: runImmediates drains a snapshot of the
  // queue, so newly queued immediates land in the next batch.
  const { promise, resolve } = Promise.withResolvers();
  const order: number[] = [];
  setImmediate(() => {
    order.push(1);
    setImmediate(() => {
      order.push(3);
      resolve(null);
    });
  });
  setImmediate(() => {
    order.push(2);
  });
  await promise;
  assertArrayEquals(order, [1, 2, 3]);
});

test(async function testClearImmediateIdempotent() {
  const { promise, resolve } = Promise.withResolvers();
  const imm = setImmediate(() => {});
  clearImmediate(imm);
  // Clearing again should be a no-op
  clearImmediate(imm);
  clearImmediate(null);
  clearImmediate(undefined);
  setImmediate(() => resolve(null));
  await promise;
});

test(async function testImmediateException() {
  const { promise, resolve } = Promise.withResolvers<Error>();
  globalThis.onerror = ((e: ErrorEvent) => {
    resolve(e.error);
    e.preventDefault();
    // deno-lint-ignore no-explicit-any
  }) as any;
  try {
    setImmediate(() => {
      throw new Error("immediate error");
    });
    assertEquals("immediate error", (await promise).message);
  } finally {
    globalThis.onerror = null;
  }
});

test(async function testUnrefImmediateFiresWithOtherWork() {
  // An unrefed immediate should still fire when other work (a timer)
  // keeps the event loop alive — matching libuv semantics where unrefed
  // handles participate in iterations that run for other reasons.
  const { promise, resolve } = Promise.withResolvers();
  let immediateFired = false;
  const imm = setImmediate(() => {
    immediateFired = true;
  });
  // deno-lint-ignore no-explicit-any
  (Deno as any).unrefImmediate(imm);
  // The timer keeps the loop alive. After it fires, check the flag.
  setTimeout(() => {
    // Use a second immediate (refed) to check after the check phase
    // of the iteration where the timer fired.
    setImmediate(() => {
      resolve(null);
    });
  }, 50);
  await promise;
  assert(
    immediateFired,
    "unrefed immediate should fire when other work keeps loop alive",
  );
});

test(async function testManyImmediates() {
  const { promise, resolve } = Promise.withResolvers();
  let count = 0;
  const total = 1000;
  for (let i = 0; i < total; i++) {
    setImmediate(() => {
      count++;
      if (count === total) resolve(null);
    });
  }
  await promise;
  assertEquals(count, total);
});
