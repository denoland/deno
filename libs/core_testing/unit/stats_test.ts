// Copyright 2018-2025 the Deno authors. MIT license.
import {
  barrierAwait,
  barrierCreate,
  LeakType,
  StatsFactory,
} from "checkin:async";
import {
  assert,
  assertEquals,
  assertStackTraceEquals,
  test,
} from "checkin:testing";

const { op_pipe_create } = Deno.core.ops;

test(async function testStatsOps() {
  using statsBefore = StatsFactory.capture();
  assert(statsBefore.dump().empty);

  barrierCreate("barrier", 3);
  const promise1 = barrierAwait("barrier");
  assertEquals(1, StatsFactory.capture().dump().count(LeakType.AsyncOp));
  const promise2 = barrierAwait("barrier");
  assertEquals(2, StatsFactory.capture().dump().count(LeakType.AsyncOp));
  // No traces here at all, even though we have ops
  assertEquals(
    0,
    StatsFactory.capture().dump().countWithTraces(LeakType.AsyncOp),
  );
  using statsMiddle = StatsFactory.capture();
  const diffMiddle = StatsFactory.diff(statsBefore, statsMiddle);
  assertEquals(0, diffMiddle.disappeared.count(LeakType.AsyncOp));
  assertEquals(2, diffMiddle.appeared.count(LeakType.AsyncOp));
  // No traces here at all, even though we have ops
  assertEquals(0, diffMiddle.appeared.countWithTraces(LeakType.AsyncOp));

  await Promise.all([promise1, promise2, barrierAwait("barrier")]);

  using statsAfter = StatsFactory.capture();
  const diff = StatsFactory.diff(statsBefore, statsAfter);
  assert(diff.empty);
});

test(function testStatsResources() {
  using statsBefore = StatsFactory.capture();

  const [p1, p2] = op_pipe_create();
  using statsMiddle = StatsFactory.capture();
  const diffMiddle = StatsFactory.diff(statsBefore, statsMiddle);
  assertEquals(0, diffMiddle.disappeared.count(LeakType.Resource));
  assertEquals(2, diffMiddle.appeared.count(LeakType.Resource));
  Deno.core.close(p1);
  Deno.core.close(p2);

  using statsAfter = StatsFactory.capture();
  const diff = StatsFactory.diff(statsBefore, statsAfter);
  assert(diff.empty);
});

test(function testTimers() {
  using statsBefore = StatsFactory.capture();

  const timeout = setTimeout(() => null, 1000);
  const interval = setInterval(() => null, 1000);

  using statsMiddle = StatsFactory.capture();
  const diffMiddle = StatsFactory.diff(statsBefore, statsMiddle);
  assertEquals(
    0,
    diffMiddle.disappeared.count(LeakType.Timer, LeakType.Interval),
  );
  assertEquals(2, diffMiddle.appeared.count(LeakType.Timer, LeakType.Interval));
  clearTimeout(timeout);
  clearInterval(interval);

  using statsAfter = StatsFactory.capture();
  const diff = StatsFactory.diff(statsBefore, statsAfter);
  assert(diff.empty);
});

async function enableTracingForTest(f: () => Promise<void> | void) {
  const oldTracingState = Deno.core.isLeakTracingEnabled();
  Deno.core.setLeakTracingEnabled(true);
  try {
    await f();
  } finally {
    Deno.core.setLeakTracingEnabled(oldTracingState);
  }
}

test(async function testAsyncLeakTrace() {
  await enableTracingForTest(async () => {
    barrierCreate("barrier", 2);
    const tracesBefore = Deno.core.getAllLeakTraces();
    using statsBefore = StatsFactory.capture();
    const p1 = barrierAwait("barrier");
    const tracesAfter = Deno.core.getAllLeakTraces();
    using statsAfter = StatsFactory.capture();
    const diff = StatsFactory.diff(statsBefore, statsAfter);
    // We don't test the contents, just that we have a trace here
    assertEquals(diff.appeared.countWithTraces(LeakType.AsyncOp), 1);

    assertEquals(tracesAfter.size, tracesBefore.size + 1);
    assertStackTraceEquals(
      Deno.core.getLeakTraceForPromise(p1)!,
      `
      at op_async_barrier_await (ext:core/00_infra.js:line:col)
      at barrierAwait (checkin:async:line:col)
      at test:///unit/stats_test.ts:line:col
      at enableTracingForTest (test:///unit/stats_test.ts:line:col)
      at testAsyncLeakTrace (test:///unit/stats_test.ts:line:col)
    `,
    );
    const p2 = barrierAwait("barrier");
    await Promise.all([p1, p2]);
  });
});

test(async function testTimeoutLeakTrace() {
  await enableTracingForTest(() => {
    const tracesBefore = Deno.core.getAllLeakTraces();
    using statsBefore = StatsFactory.capture();
    const t1 = setTimeout(() => {}, 100_000);
    const tracesAfter = Deno.core.getAllLeakTraces();
    using statsAfter = StatsFactory.capture();
    const diff = StatsFactory.diff(statsBefore, statsAfter);
    assertEquals(diff.appeared.countWithTraces(LeakType.Timer), 1);

    assertEquals(tracesAfter.size, tracesBefore.size + 1);
    clearTimeout(t1);
    const tracesFinal = Deno.core.getAllLeakTraces();
    assertEquals(tracesFinal.size, 0);
  });
});

test(async function testIntervalLeakTrace() {
  await enableTracingForTest(() => {
    const tracesBefore = Deno.core.getAllLeakTraces();
    using statsBefore = StatsFactory.capture();
    const t1 = setInterval(() => {}, 100_000);
    const tracesAfter = Deno.core.getAllLeakTraces();
    using statsAfter = StatsFactory.capture();
    const diff = StatsFactory.diff(statsBefore, statsAfter);
    assertEquals(diff.appeared.countWithTraces(LeakType.Interval), 1);

    assertEquals(tracesAfter.size, tracesBefore.size + 1);
    clearInterval(t1);
    const tracesFinal = Deno.core.getAllLeakTraces();
    assertEquals(tracesFinal.size, 0);
  });
});

test(async function testSystemTimeoutLeakTrace() {
  await enableTracingForTest(() => {
    const tracesBefore = Deno.core.getAllLeakTraces();
    using statsBefore = StatsFactory.capture();
    const t1 = Deno.core.queueSystemTimer(undefined, false, 100_000, () => {});
    const tracesAfter = Deno.core.getAllLeakTraces();
    using statsAfter = StatsFactory.capture();
    const diff = StatsFactory.diff(statsBefore, statsAfter);
    assertEquals(diff.appeared.countWithTraces(LeakType.Timer), 0);

    assertEquals(tracesAfter.size, tracesBefore.size);
    clearTimeout(t1);
    const tracesFinal = Deno.core.getAllLeakTraces();
    assertEquals(tracesFinal.size, 0);
  });
});
