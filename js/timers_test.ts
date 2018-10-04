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
  let arg = 1;
  await new Promise((resolve, reject) => {
    setTimeout(
      (a, b, c) => {
        try {
          assertEqual(a, arg);
          assertEqual(b, arg.toString());
          assertEqual(c, [arg]);
          resolve();
        } catch (e) {
          reject(e);
        }
      },
      10,
      arg,
      arg.toString(),
      [arg]
    );
  });
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
  await new Promise((resolve, reject) => {
    setTimeout(() => {
      try {
        assertEqual(timeouts, 1);
        resolve();
      } catch (e) {
        reject(e);
      }
    }, 100);
  });
});

test(async function intervalCancelInvalidSilentFail() {
  // Should silently fail (no panic)
  clearInterval(2147483647);
});
