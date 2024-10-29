// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

import {
  assert,
  assertEquals,
  assertNotEquals,
  delay,
  execCode,
  unreachable,
} from "./test_util.ts";

Deno.test(async function functionParameterBindingSuccess() {
  const { promise, resolve } = Promise.withResolvers<void>();
  let count = 0;

  const nullProto = (newCount: number) => {
    count = newCount;
    resolve();
  };

  Reflect.setPrototypeOf(nullProto, null);

  setTimeout(nullProto, 500, 1);
  await promise;
  // count should be reassigned
  assertEquals(count, 1);
});

Deno.test(async function stringifyAndEvalNonFunctions() {
  // eval can only access global scope
  const global = globalThis as unknown as {
    globalPromise: ReturnType<typeof Promise.withResolvers<void>>;
    globalCount: number;
  };

  global.globalPromise = Promise.withResolvers<void>();
  global.globalCount = 0;

  const notAFunction =
    "globalThis.globalCount++; globalThis.globalPromise.resolve();" as unknown as () =>
      void;

  setTimeout(notAFunction, 500);

  await global.globalPromise.promise;

  // count should be incremented
  assertEquals(global.globalCount, 1);

  Reflect.deleteProperty(global, "globalPromise");
  Reflect.deleteProperty(global, "globalCount");
});

Deno.test(async function timeoutSuccess() {
  const { promise, resolve } = Promise.withResolvers<void>();
  let count = 0;
  setTimeout(() => {
    count++;
    resolve();
  }, 500);
  await promise;
  // count should increment
  assertEquals(count, 1);
});

Deno.test(async function timeoutEvalNoScopeLeak() {
  // eval can only access global scope
  const global = globalThis as unknown as {
    globalPromise: ReturnType<typeof Promise.withResolvers<Error>>;
  };
  global.globalPromise = Promise.withResolvers();
  setTimeout(
    `
    try {
      console.log(core);
      globalThis.globalPromise.reject(new Error("Didn't throw."));
    } catch (error) {
      globalThis.globalPromise.resolve(error);
    }` as unknown as () => void,
    0,
  );
  const error = await global.globalPromise.promise;
  assertEquals(error.name, "ReferenceError");
  Reflect.deleteProperty(global, "globalPromise");
});

Deno.test(async function evalPrimordial() {
  const global = globalThis as unknown as {
    globalPromise: ReturnType<typeof Promise.withResolvers<void>>;
  };
  global.globalPromise = Promise.withResolvers<void>();
  const originalEval = globalThis.eval;
  let wasCalled = false;
  globalThis.eval = (argument) => {
    wasCalled = true;
    return originalEval(argument);
  };
  setTimeout(
    "globalThis.globalPromise.resolve();" as unknown as () => void,
    0,
  );
  await global.globalPromise.promise;
  assert(!wasCalled);
  Reflect.deleteProperty(global, "globalPromise");
  globalThis.eval = originalEval;
});

Deno.test(async function timeoutArgs() {
  const { promise, resolve } = Promise.withResolvers<void>();
  const arg = 1;
  let capturedArgs: unknown[] = [];
  setTimeout(
    function () {
      capturedArgs = [...arguments];
      resolve();
    },
    10,
    arg,
    arg.toString(),
    [arg],
  );
  await promise;
  assertEquals(capturedArgs, [
    arg,
    arg.toString(),
    [arg],
  ]);
});

Deno.test(async function timeoutCancelSuccess() {
  let count = 0;
  const id = setTimeout(() => {
    count++;
  }, 1);
  // Cancelled, count should not increment
  clearTimeout(id);
  await delay(600);
  assertEquals(count, 0);
});

Deno.test(async function timeoutCancelMultiple() {
  function uncalled(): never {
    throw new Error("This function should not be called.");
  }

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
  await delay(50);
});

Deno.test(async function timeoutCancelInvalidSilentFail() {
  // Expect no panic
  const { promise, resolve } = Promise.withResolvers<void>();
  let count = 0;
  const id = setTimeout(() => {
    count++;
    // Should have no effect
    clearTimeout(id);
    resolve();
  }, 500);
  await promise;
  assertEquals(count, 1);

  // Should silently fail (no panic)
  clearTimeout(2147483647);
});

Deno.test(async function intervalSuccess() {
  const { promise, resolve } = Promise.withResolvers<void>();
  let count = 0;
  const id = setInterval(() => {
    count++;
    clearInterval(id);
    resolve();
  }, 100);
  await promise;
  // Clear interval
  clearInterval(id);
  // count should increment twice
  assertEquals(count, 1);
  // Similar false async leaking alarm.
  // Force next round of polling.
  await delay(0);
});

Deno.test(async function intervalCancelSuccess() {
  let count = 0;
  const id = setInterval(() => {
    count++;
  }, 1);
  clearInterval(id);
  await delay(500);
  assertEquals(count, 0);
});

Deno.test(async function intervalOrdering() {
  const timers: number[] = [];
  let timeouts = 0;
  function onTimeout() {
    ++timeouts;
    for (let i = 1; i < timers.length; i++) {
      clearTimeout(timers[i]);
    }
  }
  for (let i = 0; i < 10; i++) {
    timers[i] = setTimeout(onTimeout, 1);
  }
  await delay(500);
  assertEquals(timeouts, 1);
});

Deno.test(function intervalCancelInvalidSilentFail() {
  // Should silently fail (no panic)
  clearInterval(2147483647);
});

// If a repeating timer is dispatched, the next interval that should first is based on
// when the timer is dispatched, not when the timer handler completes.
Deno.test(async function callbackTakesLongerThanInterval() {
  const { promise, resolve } = Promise.withResolvers<void>();
  const output: number[] = [];
  let last = 0;
  const id = setInterval(() => {
    const now = performance.now();
    if (last > 0) {
      output.push(now - last);
      if (output.length >= 10) {
        resolve();
        clearTimeout(id);
      }
    }
    last = now;
    while (performance.now() - now < 300) {
      /* hot loop */
    }
  }, 100);
  await promise;
  const total = output.reduce((t, n) => t + n, 0) / output.length;
  console.log(output);
  assert(total < 350 && total > 299, "Total was out of range: " + total);
});

// https://github.com/denoland/deno/issues/11398
Deno.test(async function clearTimeoutAfterNextTimerIsDue1() {
  const { promise, resolve } = Promise.withResolvers<void>();

  setTimeout(() => {
    resolve();
  }, 300);

  const interval = setInterval(() => {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 400);
    // Both the interval and the timeout's due times are now in the past.
    clearInterval(interval);
  }, 100);

  await promise;
});

// https://github.com/denoland/deno/issues/11398
Deno.test(async function clearTimeoutAfterNextTimerIsDue2() {
  const { promise, resolve } = Promise.withResolvers<void>();

  const timeout1 = setTimeout(unreachable, 100);

  setTimeout(() => {
    resolve();
  }, 200);

  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 300);
  // Both of the timeouts' due times are now in the past.
  clearTimeout(timeout1);

  await promise;
});

Deno.test(async function fireCallbackImmediatelyWhenDelayOverMaxValue() {
  let count = 0;
  setTimeout(() => {
    count++;
  }, 2 ** 31);
  await delay(1);
  assertEquals(count, 1);
});

Deno.test(async function timeoutCallbackThis() {
  const { promise, resolve } = Promise.withResolvers<void>();
  let capturedThis: unknown;
  const obj = {
    foo() {
      capturedThis = this;
      resolve();
    },
  };
  setTimeout(obj.foo, 1);
  await promise;
  assertEquals(capturedThis, globalThis);
});

Deno.test(async function timeoutBindThis() {
  const thisCheckPassed = [null, undefined, globalThis];

  const thisCheckFailed = [
    0,
    "",
    true,
    false,
    {},
    [],
    "foo",
    () => {},
    Object.prototype,
  ];

  for (const thisArg of thisCheckPassed) {
    const { promise, resolve } = Promise.withResolvers<void>();
    let hasThrown = 0;
    try {
      setTimeout.call(thisArg, () => resolve(), 1);
      hasThrown = 1;
    } catch (err) {
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    await promise;
    assertEquals(hasThrown, 1);
  }

  for (const thisArg of thisCheckFailed) {
    let hasThrown = 0;
    try {
      setTimeout.call(thisArg, () => {}, 1);
      hasThrown = 1;
    } catch (err) {
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEquals(hasThrown, 2);
  }
});

Deno.test(function clearTimeoutShouldConvertToNumber() {
  let called = false;
  const obj = {
    valueOf(): number {
      called = true;
      return 1;
    },
  };
  clearTimeout((obj as unknown) as number);
  assert(called);
});

Deno.test(function setTimeoutShouldThrowWithBigint() {
  let hasThrown = 0;
  try {
    setTimeout(() => {}, (1n as unknown) as number);
    hasThrown = 1;
  } catch (err) {
    if (err instanceof TypeError) {
      hasThrown = 2;
    } else {
      hasThrown = 3;
    }
  }
  assertEquals(hasThrown, 2);
});

Deno.test(function clearTimeoutShouldThrowWithBigint() {
  let hasThrown = 0;
  try {
    clearTimeout((1n as unknown) as number);
    hasThrown = 1;
  } catch (err) {
    if (err instanceof TypeError) {
      hasThrown = 2;
    } else {
      hasThrown = 3;
    }
  }
  assertEquals(hasThrown, 2);
});

Deno.test(function testFunctionName() {
  assertEquals(clearTimeout.name, "clearTimeout");
  assertEquals(clearInterval.name, "clearInterval");
});

Deno.test(function testFunctionParamsLength() {
  assertEquals(setTimeout.length, 1);
  assertEquals(setInterval.length, 1);
  assertEquals(clearTimeout.length, 0);
  assertEquals(clearInterval.length, 0);
});

Deno.test(function clearTimeoutAndClearIntervalNotBeEquals() {
  assertNotEquals(clearTimeout, clearInterval);
});

Deno.test(async function timerOrdering() {
  const array: number[] = [];
  const { promise: donePromise, resolve } = Promise.withResolvers<void>();

  function push(n: number) {
    array.push(n);
    if (array.length === 6) {
      resolve();
    }
  }

  setTimeout(() => {
    push(1);
    setTimeout(() => push(4));
  }, 0);
  setTimeout(() => {
    push(2);
    setTimeout(() => push(5));
  }, 0);
  setTimeout(() => {
    push(3);
    setTimeout(() => push(6));
  }, 0);

  await donePromise;

  assertEquals(array, [1, 2, 3, 4, 5, 6]);
});

Deno.test(async function timerBasicMicrotaskOrdering() {
  let s = "";
  let count = 0;
  const { promise, resolve } = Promise.withResolvers<void>();
  setTimeout(() => {
    Promise.resolve().then(() => {
      count++;
      s += "de";
      if (count === 2) {
        resolve();
      }
    });
  });
  setTimeout(() => {
    count++;
    s += "no";
    if (count === 2) {
      resolve();
    }
  });
  await promise;
  assertEquals(s, "deno");
});

Deno.test(async function timerNestedMicrotaskOrdering() {
  let s = "";
  const { promise, resolve } = Promise.withResolvers<void>();
  s += "0";
  setTimeout(() => {
    s += "4";
    setTimeout(() => (s += "A"));
    Promise.resolve()
      .then(() => {
        setTimeout(() => {
          s += "B";
          resolve();
        });
      })
      .then(() => {
        s += "5";
      });
  });
  setTimeout(() => (s += "6"));
  Promise.resolve().then(() => (s += "2"));
  Promise.resolve().then(() =>
    setTimeout(() => {
      s += "7";
      Promise.resolve()
        .then(() => (s += "8"))
        .then(() => {
          s += "9";
        });
    })
  );
  Promise.resolve().then(() => Promise.resolve().then(() => (s += "3")));
  s += "1";
  await promise;
  assertEquals(s, "0123456789AB");
});

Deno.test(function testQueueMicrotask() {
  assertEquals(typeof queueMicrotask, "function");
});

Deno.test(async function timerIgnoresDateOverride() {
  const OriginalDate = Date;
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  let hasThrown = 0;
  try {
    const overrideCalled: () => number = () => {
      reject("global Date override used over original Date object");
      return 0;
    };
    const DateOverride = () => {
      overrideCalled();
    };
    globalThis.Date = DateOverride as DateConstructor;
    globalThis.Date.now = overrideCalled;
    globalThis.Date.UTC = overrideCalled;
    globalThis.Date.parse = overrideCalled;
    queueMicrotask(() => {
      resolve();
    });
    await promise;
    hasThrown = 1;
  } catch (err) {
    if (typeof err === "string") {
      assertEquals(err, "global Date override used over original Date object");
      hasThrown = 2;
    } else if (err instanceof TypeError) {
      hasThrown = 3;
    } else {
      hasThrown = 4;
    }
  } finally {
    globalThis.Date = OriginalDate;
  }
  assertEquals(hasThrown, 1);
});

Deno.test({
  name: "unrefTimer",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      const timer = setTimeout(() => console.log("1"), 1);
      Deno.unrefTimer(timer);
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "");
  },
});

Deno.test({
  name: "unrefTimer - mix ref and unref 1",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      const timer1 = setTimeout(() => console.log("1"), 200);
      const timer2 = setTimeout(() => console.log("2"), 400);
      const timer3 = setTimeout(() => console.log("3"), 600);
      Deno.unrefTimer(timer3);
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "1\n2\n");
  },
});

Deno.test({
  name: "unrefTimer - mix ref and unref 2",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      const timer1 = setTimeout(() => console.log("1"), 200);
      const timer2 = setTimeout(() => console.log("2"), 400);
      const timer3 = setTimeout(() => console.log("3"), 600);
      Deno.unrefTimer(timer1);
      Deno.unrefTimer(timer2);
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "1\n2\n3\n");
  },
});

Deno.test({
  name: "unrefTimer - unref interval",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      let i = 0;
      const timer1 = setInterval(() => {
        console.log("1");
        i++;
        if (i === 5) {
          Deno.unrefTimer(timer1);
        }
      }, 10);
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "1\n1\n1\n1\n1\n");
  },
});

Deno.test({
  name: "unrefTimer - unref then ref 1",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      const timer1 = setTimeout(() => console.log("1"), 10);
      Deno.unrefTimer(timer1);
      Deno.refTimer(timer1);
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "1\n");
  },
});

Deno.test({
  name: "unrefTimer - unref then ref",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      const timer1 = setTimeout(() => {
        console.log("1");
        Deno.refTimer(timer2);
      }, 10);
      const timer2 = setTimeout(() => console.log("2"), 20);
      Deno.unrefTimer(timer2);
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "1\n2\n");
  },
});

Deno.test({
  name: "unrefTimer - invalid calls do nothing",
  fn: () => {
    Deno.unrefTimer(NaN);
    Deno.refTimer(NaN);
  },
});

Deno.test({
  name: "AbortSignal.timeout() with no listeners",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      const signal = AbortSignal.timeout(2000);

      // This unref timer expires before the signal, and if it does expire, then
      // it means the signal has kept the event loop alive.
      const timer = setTimeout(() => console.log("Unexpected!"), 1500);
      Deno.unrefTimer(timer);
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "");
  },
});

Deno.test({
  name: "AbortSignal.timeout() with listeners",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      const signal = AbortSignal.timeout(1000);
      signal.addEventListener("abort", () => console.log("Event fired!"));
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "Event fired!\n");
  },
});

Deno.test({
  name: "AbortSignal.timeout() with removed listeners",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      const signal = AbortSignal.timeout(2000);

      const callback = () => console.log("Unexpected: Event fired");
      signal.addEventListener("abort", callback);

      setTimeout(() => {
        console.log("Removing the listener.");
        signal.removeEventListener("abort", callback);
      }, 500);

      Deno.unrefTimer(
        setTimeout(() => console.log("Unexpected: Unref timer"), 1500)
      );
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "Removing the listener.\n");
  },
});

Deno.test({
  name: "AbortSignal.timeout() with listener for a non-abort event",
  permissions: { run: true, read: true },
  fn: async () => {
    const [statusCode, output] = await execCode(`
      const signal = AbortSignal.timeout(2000);

      signal.addEventListener("someOtherEvent", () => {
        console.log("Unexpected: someOtherEvent called");
      });

      Deno.unrefTimer(
        setTimeout(() => console.log("Unexpected: Unref timer"), 1500)
      );
    `);
    assertEquals(statusCode, 0);
    assertEquals(output, "");
  },
});

// Regression test for https://github.com/denoland/deno/issues/19866
Deno.test({
  name: "regression for #19866",
  fn: async () => {
    const timeoutsFired = [];

    // deno-lint-ignore require-await
    async function start(n: number) {
      let i = 0;
      const intervalId = setInterval(() => {
        i++;
        if (i > 2) {
          clearInterval(intervalId!);
        }
        timeoutsFired.push(n);
      }, 20);
    }

    for (let n = 0; n < 100; n++) {
      start(n);
    }

    // 3s should be plenty of time for all the intervals to fire
    // but it might still be flaky on CI.
    await new Promise((resolve) => setTimeout(resolve, 3000));
    assertEquals(timeoutsFired.length, 300);
  },
});

// Regression test for https://github.com/denoland/deno/issues/20367
Deno.test({
  name: "regression for #20367",
  fn: async () => {
    const { promise, resolve } = Promise.withResolvers<number>();
    const start = performance.now();
    setTimeout(() => {
      const end = performance.now();
      resolve(end - start);
    }, 1000);
    clearTimeout(setTimeout(() => {}, 1000));

    const result = await promise;
    assert(result >= 1000);
  },
});

// Regression test for https://github.com/denoland/deno/issues/20663
Deno.test({
  name: "regression for #20663",
  fn: () => {
    AbortSignal.timeout(2000);
  },
});
