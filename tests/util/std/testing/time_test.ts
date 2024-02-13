// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertInstanceOf,
  assertNotEquals,
  assertRejects,
  assertStrictEquals,
} from "../assert/mod.ts";
import { FakeTime, TimeError } from "./time.ts";
import { _internals } from "./_time.ts";
import { assertSpyCall, spy, SpyCall } from "./mock.ts";

function fromNow(): () => number {
  const start: number = Date.now();
  return () => Date.now() - start;
}

Deno.test("Date unchanged if FakeTime is uninitialized", () => {
  assertStrictEquals(Date, _internals.Date);
});

Deno.test("Date is fake if FakeTime is initialized", () => {
  const time = new FakeTime(9001);
  try {
    assertNotEquals(Date, _internals.Date);
  } finally {
    time.restore();
  }
  assertStrictEquals(Date, _internals.Date);
});

Deno.test("Fake Date parse and UTC behave the same", () => {
  const expectedUTC = Date.UTC(96, 1, 2, 3, 4, 5);
  const expectedParse = Date.parse("04 Dec 1995 00:12:00 GMT");

  const time = new FakeTime();
  try {
    assertEquals(
      Date.UTC(96, 1, 2, 3, 4, 5),
      expectedUTC,
    );
    assertEquals(
      Date.parse("04 Dec 1995 00:12:00 GMT"),
      expectedParse,
    );
  } finally {
    time.restore();
  }
});

Deno.test("Fake Date.now returns current fake time", () => {
  const time: FakeTime = new FakeTime(9001);
  const now = spy(_internals.Date, "now");
  try {
    try {
      assertEquals(Date.now(), 9001);
      assertEquals(now.calls.length, 0);
      time.tick(1523);
      assertEquals(Date.now(), 10524);
      assertEquals(now.calls.length, 0);
    } finally {
      time.restore();
    }
    assertNotEquals(Date.now(), 10524);
    assertEquals(now.calls.length, 1);
  } finally {
    now.restore();
  }
});

Deno.test("Fake Date instance methods passthrough to real Date instance methods", () => {
  const time = new FakeTime();
  try {
    const now = new Date("2020-05-25T05:00:00.12345Z");
    assertEquals(now.toISOString(), "2020-05-25T05:00:00.123Z");

    const func1 = spy(
      _internals.Date.prototype,
      "toISOString",
    );
    try {
      now.toISOString();
      assertSpyCall(func1, 0, {
        args: [],
        returned: "2020-05-25T05:00:00.123Z",
      });
      assertInstanceOf(func1.calls[0].self, _internals.Date);
    } finally {
      func1.restore();
    }

    const func2 = spy(
      _internals.Date.prototype,
      Symbol.toPrimitive,
    );
    try {
      Number(now);
      assertSpyCall(func2, 0, {
        args: ["number"],
        returned: 1590382800123,
      });
    } finally {
      func2.restore();
    }
  } finally {
    time.restore();
  }
});

Deno.test("timeout functions unchanged if FakeTime is uninitialized", () => {
  assertStrictEquals(setTimeout, _internals.setTimeout);
  assertStrictEquals(clearTimeout, _internals.clearTimeout);
});

Deno.test("timeout functions are fake if FakeTime is initialized", () => {
  const time: FakeTime = new FakeTime();
  try {
    assertNotEquals(setTimeout, _internals.setTimeout);
    assertNotEquals(clearTimeout, _internals.clearTimeout);
  } finally {
    time.restore();
  }
  assertStrictEquals(setTimeout, _internals.setTimeout);
  assertStrictEquals(clearTimeout, _internals.clearTimeout);
});

Deno.test("FakeTime only ticks forward when setting now or calling tick", () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();

  try {
    assertEquals(Date.now(), start);
    time.tick(5);
    assertEquals(Date.now(), start + 5);
    time.now = start + 1000;
    assertEquals(Date.now(), start + 1000);
    assert(_internals.Date.now() < start + 1000);
  } finally {
    time.restore();
  }
});

Deno.test("FakeTime controls timeouts", () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();
  const cb = spy(fromNow());
  const expected: SpyCall[] = [];

  try {
    setTimeout(cb, 1000);
    time.tick(250);
    assertEquals(cb.calls, expected);
    time.tick(250);
    assertEquals(cb.calls, expected);
    time.tick(500);
    expected.push({ args: [], returned: 1000 });
    assertEquals(cb.calls, expected);
    time.tick(2500);
    assertEquals(cb.calls, expected);

    setTimeout(cb, 1000, "a");
    setTimeout(cb, 2000, "b");
    setTimeout(cb, 1500, "c");
    assertEquals(cb.calls, expected);
    time.tick(2500);
    expected.push({ args: ["a"], returned: 4500 });
    expected.push({ args: ["c"], returned: 5000 });
    expected.push({ args: ["b"], returned: 5500 });
    assertEquals(cb.calls, expected);

    setTimeout(cb, 1000, "a");
    setTimeout(cb, 1500, "b");
    const timeout: number = setTimeout(cb, 1750, "c");
    setTimeout(cb, 2000, "d");
    time.tick(1250);
    expected.push({ args: ["a"], returned: 7000 });
    assertEquals(cb.calls, expected);
    assertEquals(Date.now(), start + 7250);
    clearTimeout(timeout);
    time.tick(500);
    expected.push({ args: ["b"], returned: 7500 });
    assertEquals(cb.calls, expected);
    assertEquals(Date.now(), start + 7750);
    time.tick(250);
    expected.push({ args: ["d"], returned: 8000 });
    assertEquals(cb.calls, expected);
  } finally {
    time.restore();
  }
});

Deno.test("interval functions unchanged if FakeTime is uninitialized", () => {
  assertStrictEquals(setInterval, _internals.setInterval);
  assertStrictEquals(clearInterval, _internals.clearInterval);
});

Deno.test("interval functions are fake if FakeTime is initialized", () => {
  const time: FakeTime = new FakeTime();
  try {
    assertNotEquals(setInterval, _internals.setInterval);
    assertNotEquals(clearInterval, _internals.clearInterval);
  } finally {
    time.restore();
  }
  assertStrictEquals(setInterval, _internals.setInterval);
  assertStrictEquals(clearInterval, _internals.clearInterval);
});

Deno.test("FakeTime controls intervals", () => {
  const time: FakeTime = new FakeTime();
  const cb = spy(fromNow());
  const expected: SpyCall[] = [];
  try {
    const interval: number = setInterval(cb, 1000);
    time.tick(250);
    assertEquals(cb.calls, expected);
    time.tick(250);
    assertEquals(cb.calls, expected);
    time.tick(500);
    expected.push({ args: [], returned: 1000 });
    assertEquals(cb.calls, expected);
    time.tick(2500);
    expected.push({ args: [], returned: 2000 });
    expected.push({ args: [], returned: 3000 });
    assertEquals(cb.calls, expected);

    clearInterval(interval);
    time.tick(1000);
    assertEquals(cb.calls, expected);
  } finally {
    time.restore();
  }
});

Deno.test("FakeTime calls timeout and interval callbacks in correct order", () => {
  const time: FakeTime = new FakeTime();
  const cb = spy(fromNow());
  const timeoutCb = spy(cb);
  const intervalCb = spy(cb);
  const expected: SpyCall[] = [];
  const timeoutExpected: SpyCall[] = [];
  const intervalExpected: SpyCall[] = [];
  try {
    const interval: number = setInterval(intervalCb, 1000);
    setTimeout(timeoutCb, 500);
    time.tick(250);
    assertEquals(intervalCb.calls, intervalExpected);
    time.tick(250);
    setTimeout(timeoutCb, 1000);
    let expect: SpyCall = { args: [], returned: 500 };
    expected.push(expect);
    timeoutExpected.push(expect);
    assertEquals(cb.calls, expected);
    assertEquals(timeoutCb.calls, timeoutExpected);
    assertEquals(cb.calls, expected);
    assertEquals(intervalCb.calls, intervalExpected);
    time.tick(500);
    expect = { args: [], returned: 1000 };
    expected.push(expect);
    intervalExpected.push(expect);
    assertEquals(cb.calls, expected);
    assertEquals(intervalCb.calls, intervalExpected);
    time.tick(2500);
    expect = { args: [], returned: 1500 };
    expected.push(expect);
    timeoutExpected.push(expect);
    expect = { args: [], returned: 2000 };
    expected.push(expect);
    intervalExpected.push(expect);
    expect = { args: [], returned: 3000 };
    expected.push(expect);
    intervalExpected.push(expect);
    assertEquals(cb.calls, expected);
    assertEquals(timeoutCb.calls, timeoutExpected);
    assertEquals(intervalCb.calls, intervalExpected);

    clearInterval(interval);
    time.tick(1000);
    assertEquals(cb.calls, expected);
    assertEquals(timeoutCb.calls, timeoutExpected);
    assertEquals(intervalCb.calls, intervalExpected);
  } finally {
    time.restore();
  }
});

Deno.test("FakeTime restoreFor restores real time temporarily", async () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();

  try {
    assertEquals(Date.now(), start);
    time.tick(1000);
    assertEquals(Date.now(), start + 1000);
    assert(_internals.Date.now() < start + 1000);
    await FakeTime.restoreFor(() => {
      assert(Date.now() < start + 1000);
    });
    assertEquals(Date.now(), start + 1000);
    assert(_internals.Date.now() < start + 1000);
  } finally {
    time.restore();
  }
});

Deno.test("FakeTime restoreFor restores real time and re-overridden atomically", async () => {
  const time: FakeTime = new FakeTime();
  const fakeSetTimeout = setTimeout;
  const actualSetTimeouts: (typeof setTimeout)[] = [];

  try {
    const asyncFn = async () => {
      actualSetTimeouts.push(setTimeout);
      await Promise.resolve();
      actualSetTimeouts.push(setTimeout);
      await Promise.resolve();
      actualSetTimeouts.push(setTimeout);
    };
    const promise = asyncFn();
    await new Promise((resolve) => {
      FakeTime.restoreFor(() => setTimeout(resolve, 0));
    });
    await promise;
    assertEquals(actualSetTimeouts, [
      fakeSetTimeout,
      fakeSetTimeout,
      fakeSetTimeout,
    ]);
  } finally {
    time.restore();
  }
});

Deno.test("FakeTime restoreFor returns promise that resolved to result of callback", async () => {
  const time: FakeTime = new FakeTime();

  try {
    const resultSync = await FakeTime.restoreFor(() => "a");
    assertEquals(resultSync, "a");
    const resultAsync = await FakeTime.restoreFor(() => Promise.resolve("b"));
    assertEquals(resultAsync, "b");
  } finally {
    time.restore();
  }
});

Deno.test("FakeTime restoreFor returns promise that rejected to error in callback", async () => {
  const time: FakeTime = new FakeTime();

  try {
    await assertRejects(
      () =>
        FakeTime.restoreFor(() => {
          throw new Error("Error in sync callback");
        }),
      Error,
      "Error in sync callback",
    );
    await assertRejects(
      () =>
        FakeTime.restoreFor(() => {
          return Promise.reject(new Error("Error in async callback"));
        }),
      Error,
      "Error in async callback",
    );
  } finally {
    time.restore();
  }
});

Deno.test("FakeTime restoreFor returns promise that rejected to TimeError if FakeTime is uninitialized", async () => {
  await assertRejects(
    () => FakeTime.restoreFor(() => {}),
    TimeError,
    "no fake time",
  );
});

Deno.test("delay uses real time", async () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();

  try {
    assertEquals(Date.now(), start);
    await time.delay(20);
    assert(_internals.Date.now() >= start + 20);
    assertEquals(Date.now(), start);
  } finally {
    time.restore();
  }
});

Deno.test("delay runs all microtasks before resolving", async () => {
  const time: FakeTime = new FakeTime();

  try {
    const seq = [];
    queueMicrotask(() => seq.push(2));
    queueMicrotask(() => seq.push(3));
    seq.push(1);
    await time.delay(20);
    seq.push(4);
    assertEquals(seq, [1, 2, 3, 4]);
  } finally {
    time.restore();
  }
});

Deno.test("delay with abort", async () => {
  const time: FakeTime = new FakeTime();

  try {
    const seq = [];
    const abort = new AbortController();
    const { signal } = abort;
    const delayedPromise = time.delay(100, { signal });
    seq.push(1);
    await FakeTime.restoreFor(() => {
      setTimeout(() => {
        seq.push(2);
        abort.abort();
      }, 0);
    });
    await assertRejects(
      () => delayedPromise,
      DOMException,
      "Delay was aborted",
    );
    seq.push(3);
    assertEquals(seq, [1, 2, 3]);
  } finally {
    time.restore();
  }
});

Deno.test("runMicrotasks runs all microtasks before resolving", async () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();

  try {
    const seq = [];
    queueMicrotask(() => seq.push(2));
    queueMicrotask(() => seq.push(3));
    seq.push(1);
    await time.runMicrotasks();
    seq.push(4);
    assertEquals(seq, [1, 2, 3, 4]);
    assertEquals(Date.now(), start);
  } finally {
    time.restore();
  }
});

Deno.test("tickAsync runs all microtasks and runs timers if ticks past due", async () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();
  const cb = spy(fromNow());
  const expected: SpyCall[] = [];
  const seq: number[] = [];

  try {
    setTimeout(cb, 1000);
    queueMicrotask(() => seq.push(2));
    queueMicrotask(() => seq.push(3));
    seq.push(1);
    await time.tickAsync(250);
    seq.push(4);
    assertEquals(cb.calls, expected);
    await time.tickAsync(250);
    assertEquals(cb.calls, expected);
    queueMicrotask(() => seq.push(6));
    seq.push(5);
    await time.tickAsync(500);
    seq.push(7);
    expected.push({ args: [], returned: 1000 });
    assertEquals(cb.calls, expected);
    assertEquals(Date.now(), start + 1000);
    assertEquals(seq, [1, 2, 3, 4, 5, 6, 7]);
  } finally {
    time.restore();
  }
});

Deno.test("next runs next timer without running microtasks", async () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();
  const cb = spy(fromNow());
  const seq: number[] = [];

  try {
    // Callback is called by `next`.
    setTimeout(cb, 1000);
    queueMicrotask(() => seq.push(3));
    queueMicrotask(() => seq.push(4));
    seq.push(1);
    const hasNextTimer = time.next();
    seq.push(2);

    assertEquals(hasNextTimer, true);
    const expectedCalls = [{ args: [] as [], returned: 1000 }];
    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1000);
    await time.runMicrotasks();

    // Callback is already called before `next` called.
    queueMicrotask(() => seq.push(7));
    queueMicrotask(() => seq.push(8));
    seq.push(5);
    const hasNextTimerAfterCalled = time.next();
    seq.push(6);
    await time.runMicrotasks();

    assertEquals(hasNextTimerAfterCalled, false);
    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1000);

    // Callbacks are cleared before `next` called.
    const id1 = setTimeout(cb, 1000);
    const id2 = setTimeout(cb, 2000);
    clearTimeout(id1);
    clearTimeout(id2);
    queueMicrotask(() => seq.push(11));
    queueMicrotask(() => seq.push(12));
    seq.push(9);
    const hasNextTimerAfterCleared = time.next();
    seq.push(10);
    await time.runMicrotasks();

    assertEquals(hasNextTimerAfterCleared, false);
    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1000);

    // Callback is partially cleared before `next` called.
    const id3 = setTimeout(cb, 1500);
    setTimeout(cb, 1500);
    clearTimeout(id3);
    queueMicrotask(() => seq.push(15));
    queueMicrotask(() => seq.push(16));
    seq.push(13);
    const hasNextTimerAfterPartiallyCleared = time.next();
    seq.push(14);
    await time.runMicrotasks();

    assertEquals(hasNextTimerAfterPartiallyCleared, true);
    const expectedCalls2 = [
      { args: [] as [], returned: 1000 },
      { args: [] as [], returned: 1000 + 1500 },
    ];
    assertEquals(cb.calls, expectedCalls2);
    assertEquals(Date.now(), start + 1000 + 1500);

    assertEquals(seq, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
  } finally {
    time.restore();
  }
});

Deno.test("nextAsync runs all microtasks and next timer", async () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();
  const cb = spy(fromNow());
  const seq: number[] = [];

  try {
    // Callback is called by `nextAsync`.
    setTimeout(cb, 1000);
    queueMicrotask(() => seq.push(2));
    queueMicrotask(() => seq.push(3));
    seq.push(1);
    const hasNextTimer = await time.nextAsync();
    seq.push(4);

    assertEquals(hasNextTimer, true);
    const expectedCalls = [{ args: [] as [], returned: 1000 }];
    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1000);

    // Callback is already called before `nextAsync` called.
    queueMicrotask(() => seq.push(6));
    queueMicrotask(() => seq.push(7));
    seq.push(5);
    const hasNextTimerAfterCalled = await time.nextAsync();
    seq.push(8);

    assertEquals(hasNextTimerAfterCalled, false);
    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1000);

    // Callbacks are cleared before `nextAsync` called.
    const id1 = setTimeout(cb, 1000);
    const id2 = setTimeout(cb, 2000);
    clearTimeout(id1);
    clearTimeout(id2);
    queueMicrotask(() => seq.push(10));
    queueMicrotask(() => seq.push(11));
    seq.push(9);
    const hasNextTimerAfterCleared = await time.nextAsync();
    seq.push(12);

    assertEquals(hasNextTimerAfterCleared, false);
    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1000);

    // Callback is partially cleared before `nextAsync` called.
    const id3 = setTimeout(cb, 1500);
    setTimeout(cb, 1500);
    clearTimeout(id3);
    queueMicrotask(() => seq.push(14));
    queueMicrotask(() => seq.push(15));
    seq.push(13);
    const hasNextTimerAfterPartiallyCleared = await time.nextAsync();
    seq.push(16);
    await time.runMicrotasks();

    assertEquals(hasNextTimerAfterPartiallyCleared, true);
    const expectedCalls2 = [
      { args: [] as [], returned: 1000 },
      { args: [] as [], returned: 1000 + 1500 },
    ];
    assertEquals(cb.calls, expectedCalls2);
    assertEquals(Date.now(), start + 1000 + 1500);

    assertEquals(seq, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
  } finally {
    time.restore();
  }
});

Deno.test("runAll runs all timers without running microtasks", async () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();
  const cb = spy(fromNow());
  const seq: number[] = [];

  try {
    setTimeout(cb, 1000);
    setTimeout(cb, 1500);
    queueMicrotask(() => seq.push(3));
    queueMicrotask(() => seq.push(4));
    seq.push(1);
    time.runAll();
    seq.push(2);
    const expectedCalls = [
      { args: [] as [], returned: 1000 },
      { args: [] as [], returned: 1500 },
    ];
    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1500);
    await time.runMicrotasks();

    queueMicrotask(() => seq.push(7));
    queueMicrotask(() => seq.push(8));
    seq.push(5);
    time.runAll();
    seq.push(6);
    await time.runMicrotasks();

    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1500);
    assertEquals(seq, [1, 2, 3, 4, 5, 6, 7, 8]);
  } finally {
    time.restore();
  }
});

Deno.test("runAllAsync runs all microtasks and timers", async () => {
  const time: FakeTime = new FakeTime();
  const start: number = Date.now();
  const cb = spy(fromNow());
  const seq: number[] = [];

  try {
    setTimeout(cb, 1000);
    setTimeout(cb, 1500);
    queueMicrotask(() => seq.push(2));
    queueMicrotask(() => seq.push(3));
    seq.push(1);
    await time.runAllAsync();
    seq.push(4);
    const expectedCalls = [
      { args: [] as [], returned: 1000 },
      { args: [] as [], returned: 1500 },
    ];
    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1500);

    queueMicrotask(() => seq.push(6));
    queueMicrotask(() => seq.push(7));
    seq.push(5);
    await time.runAllAsync();
    seq.push(8);

    assertEquals(cb.calls, expectedCalls);
    assertEquals(Date.now(), start + 1500);
    assertEquals(seq, [1, 2, 3, 4, 5, 6, 7, 8]);
  } finally {
    time.restore();
  }
});

const Date_ = Date;

Deno.test("Date from FakeTime is structured cloneable", () => {
  const time: FakeTime = new FakeTime();
  try {
    const date: Date = new Date();
    assert(date instanceof Date);
    assert(date instanceof Date_);
    const cloned: Date = structuredClone(date);
    assertEquals(cloned.getTime(), date.getTime());
    assert(date instanceof Date);
    assert(cloned instanceof Date_);
  } finally {
    time.restore();
  }
});
