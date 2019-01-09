// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assertEqual } from "./test_util.ts";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return {
    promise,
    resolve,
    reject
  };
}

function waitForMs(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

test(async function timeoutSuccess() {
  const { promise, resolve } = deferred();
  let count = 0;
  setTimeout(() => {
    count++;
    resolve();
  }, 500);
  await promise;
  // count should increment
  assertEqual(count, 1);
});

test(async function timeoutArgs() {
  const { promise, resolve } = deferred();
  const arg = 1;
  setTimeout(
    (a, b, c) => {
      assertEqual(a, arg);
      assertEqual(b, arg.toString());
      assertEqual(c, [arg]);
      resolve();
    },
    10,
    arg,
    arg.toString(),
    [arg]
  );
  await promise;
});

test(async function timeoutCancelSuccess() {
  let count = 0;
  const id = setTimeout(() => {
    count++;
  }, 500);
  // Cancelled, count should not increment
  clearTimeout(id);
  // Wait a bit longer than 500ms
  await waitForMs(600);
  assertEqual(count, 0);
});

test(async function timeoutCancelMultiple() {
  // Set timers and cancel them in the same order.
  const t1 = setTimeout(uncalled, 10);
  const t2 = setTimeout(uncalled, 10);
  const t3 = setTimeout(uncalled, 10);
  clearTimeout(t1);
  clearTimeout(t2);
  clearTimeout(t3);

  // Set timers and cancel them in reverse order.
  const t4 = setTimeout(uncalled, 20);
  const t5 = setTimeout(uncalled, 20);
  const t6 = setTimeout(uncalled, 20);
  clearTimeout(t6);
  clearTimeout(t5);
  clearTimeout(t4);

  // Sleep until we're certain that the cancelled timers aren't gonna fire.
  await waitForMs(50);

  function uncalled() {
    throw new Error("This function should not be called.");
  }
});

test(async function timeoutCancelInvalidSilentFail() {
  // Expect no panic
  const { promise, resolve } = deferred();
  let count = 0;
  const id = setTimeout(() => {
    count++;
    // Should have no effect
    clearTimeout(id);
    resolve();
  }, 500);
  await promise;
  assertEqual(count, 1);

  // Should silently fail (no panic)
  clearTimeout(2147483647);
});

test(async function intervalSuccess() {
  const { promise, resolve } = deferred();
  let count = 0;
  const id = setInterval(() => {
    count++;
    if (count === 2) {
      // TODO: clearInterval(id) here alone seems not working
      // causing unit_tests.ts to block forever
      // Requires further investigation...
      clearInterval(id);
      resolve();
    }
  }, 200);
  await promise;
  // Clear interval
  clearInterval(id);
  // count should increment twice
  assertEqual(count, 2);
});

test(async function intervalCancelSuccess() {
  let count = 0;
  const id = setInterval(() => {
    count++;
  }, 500);
  // Cancelled, count should not increment
  clearInterval(id);
  // Wait a bit longer than 500ms
  await waitForMs(600);
  assertEqual(count, 0);
});

test(async function intervalOrdering() {
  const timers = [];
  let timeouts = 0;
  for (let i = 0; i < 10; i++) {
    timers[i] = setTimeout(onTimeout, 20);
  }
  function onTimeout() {
    ++timeouts;
    for (let i = 1; i < timers.length; i++) {
      clearTimeout(timers[i]);
    }
  }
  await waitForMs(100);
  assertEqual(timeouts, 1);
});

test(async function intervalCancelInvalidSilentFail() {
  // Should silently fail (no panic)
  clearInterval(2147483647);
});
