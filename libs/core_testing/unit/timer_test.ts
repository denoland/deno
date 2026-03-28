// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals, test } from "checkin:testing";

test(async function testTimeout() {
  const { promise, resolve } = Promise.withResolvers();
  const timer = setTimeout(() => {
    resolve(null);
  }, 1);
  assert(timer != null);
  await promise;
});

test(async function testTimeoutWithPromise() {
  await new Promise((r) => setTimeout(r, 1));
});

test(async function testManyTimers() {
  let n = 0;
  for (let i = 0; i < 1e6; i++) {
    setTimeout(() => n++, 1);
  }
  await new Promise((r) => setTimeout(r, 2));
  assertEquals(n, 1e6);
});

test(async function testManyIntervals() {
  const expected = 1000;
  let n = 0;
  // deno-lint-ignore no-explicit-any
  const intervals: any[] = [];
  for (let i = 0; i < 100; i++) {
    let count = 0;
    const timer = setInterval(() => {
      if (count++ == 10) {
        clearInterval(timer);
      } else {
        n++;
      }
      assert(n <= expected, `${n} <= ${expected}`);
    }, 1);
    intervals.push(timer);
  }
  // deno-lint-ignore no-explicit-any
  let checker: any;
  await new Promise((r) =>
    checker = setInterval(() => {
      assert(n <= expected, `${n} <= ${expected}`);
      if (n == expected) {
        clearInterval(checker);
        r(null);
      }
    }, 1)
  );
  assertEquals(n, expected);
});

// The timers must drain the microtask queue before attempting to run the
// next timer.
test(async function testMicrotaskOrdering() {
  const { promise, resolve } = Promise.withResolvers();
  let s = "";
  let i = 0;
  setTimeout(() => {
    Promise.resolve().then(() => {
      s += "promise\n";
    });
    if (++i == 2) {
      resolve(0);
    }
  });
  setTimeout(() => {
    s += "no promise\n";
    if (++i == 2) {
      resolve(0);
    }
  });
  await promise;
  assertEquals(s, "promise\nno promise\n");
});

test(async function testTimerException() {
  const { promise, resolve } = Promise.withResolvers<Error>();
  globalThis.onerror = ((e: ErrorEvent) => {
    resolve(e.error);
    e.preventDefault();
    // deno-lint-ignore no-explicit-any
  }) as any;
  try {
    setTimeout(() => {
      throw new Error("timeout error");
    });
    assertEquals("timeout error", (await promise).message);
  } finally {
    globalThis.onerror = null;
  }
});

test(async function testCancellationDuringDispatch() {
  // deno-lint-ignore no-explicit-any
  const timers: any[] = [];
  let timeouts = 0;

  // If a timer is ready to be dispatched, but is cancelled during the
  // dispatch of a previous timer that is also ready, that timer should
  // not be dispatched.
  function onTimeout() {
    ++timeouts;
    for (let i = 1; i < timers.length; i++) {
      clearTimeout(timers[i]);
    }
  }
  for (let i = 0; i < 10; i++) {
    timers[i] = setTimeout(onTimeout, 1);
  }
  await new Promise((r) => setTimeout(r, 10));
  assertEquals(timeouts, 1);
});
