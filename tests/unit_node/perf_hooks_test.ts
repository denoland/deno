// Copyright 2018-2026 the Deno authors. MIT license.
import {
  createHistogram,
  monitorEventLoopDelay,
  performance,
  PerformanceEntry,
  PerformanceObserver,
} from "node:perf_hooks";
import { assert, assertEquals, assertThrows } from "@std/assert";

// Basic performance API tests removed - covered by Node compat tests:
// - parallel/test-performance-global.js
// - parallel/test-performanceobserver-gc.js

Deno.test({
  name: "[perf_hooks] performance.timeOrigin",
  fn() {
    assertEquals(typeof performance.timeOrigin, "number");
    assertThrows(() => {
      // @ts-expect-error: Cannot assign to 'timeOrigin' because it is a read-only property
      performance.timeOrigin = 1;
    });
  },
});

Deno.test("[perf_hooks]: eventLoopUtilization", () => {
  const obj = performance.eventLoopUtilization();
  assertEquals(typeof obj.idle, "number");
  assertEquals(typeof obj.active, "number");
  assertEquals(typeof obj.utilization, "number");
});

Deno.test("[perf_hooks]: monitorEventLoopDelay", async () => {
  const e = monitorEventLoopDelay();
  assertEquals(e.count, 0);
  e.enable();

  await new Promise((resolve) => setTimeout(resolve, 100));

  assert(e.min > 0);
  assert(e.minBigInt > 0n);
  assert(e.count > 0);

  e.disable();
});

Deno.test("[perf_hooks]: markResourceTiming", () => {
  assert(typeof performance.markResourceTiming === "function");
});

Deno.test("[perf_hooks]: PerformanceObserver.supportedEntryTypes", () => {
  const supported = PerformanceObserver.supportedEntryTypes;
  assert(Array.isArray(supported));
  assert(supported.includes("mark"));
  assert(supported.includes("measure"));
});

Deno.test("[perf_hooks]: PerformanceObserver observes marks", async () => {
  const entries: PerformanceEntry[] = [];
  const observer = new PerformanceObserver((list) => {
    entries.push(...list.getEntries());
  });
  observer.observe({ entryTypes: ["mark"] });

  performance.mark("test-mark-1");
  performance.mark("test-mark-2");

  // Wait for microtask queue to flush
  await new Promise((resolve) => setTimeout(resolve, 10));

  assertEquals(entries.length, 2);
  assertEquals(entries[0].name, "test-mark-1");
  assertEquals(entries[1].name, "test-mark-2");
  assertEquals(entries[0].entryType, "mark");

  observer.disconnect();
  performance.clearMarks();
});

Deno.test("[perf_hooks]: PerformanceObserver observes measures", async () => {
  const entries: PerformanceEntry[] = [];
  const observer = new PerformanceObserver((list) => {
    entries.push(...list.getEntries());
  });
  observer.observe({ entryTypes: ["measure"] });

  performance.mark("start");
  performance.measure("test-measure", "start");

  await new Promise((resolve) => setTimeout(resolve, 10));

  assertEquals(entries.length, 1);
  assertEquals(entries[0].name, "test-measure");
  assertEquals(entries[0].entryType, "measure");

  observer.disconnect();
  performance.clearMarks();
  performance.clearMeasures();
});

Deno.test("[perf_hooks]: PerformanceObserver disconnect stops observation", async () => {
  const entries: PerformanceEntry[] = [];
  const observer = new PerformanceObserver((list) => {
    entries.push(...list.getEntries());
  });
  observer.observe({ entryTypes: ["mark"] });

  performance.mark("before-disconnect");
  await new Promise((resolve) => setTimeout(resolve, 10));

  observer.disconnect();

  performance.mark("after-disconnect");
  await new Promise((resolve) => setTimeout(resolve, 10));

  assertEquals(entries.length, 1);
  assertEquals(entries[0].name, "before-disconnect");

  performance.clearMarks();
});

Deno.test("[perf_hooks]: createHistogram is exported", () => {
  assertEquals(typeof createHistogram, "function");
});

Deno.test("[perf_hooks]: createHistogram default options", () => {
  const h = createHistogram();
  assertEquals(h.count, 0);
  h.record(123);
  assertEquals(h.count, 1);
  assertEquals(h.min, 123);
  assertEquals(h.max, 123);
  assertEquals(h.mean, 123);
  assertEquals(h.exceeds, 0);
  assertEquals(typeof h.stddev, "number");
});

Deno.test("[perf_hooks]: createHistogram custom bounds", () => {
  const h = createHistogram({ lowest: 1, highest: 1000, figures: 3 });
  h.record(1);
  h.record(10);
  h.record(100);
  h.record(1000);
  assertEquals(h.count, 4);
  assertEquals(h.min, 1);
  assertEquals(h.max <= 1000 && h.max >= 1000 - 1, true);
});

Deno.test("[perf_hooks]: createHistogram bigint accessors", () => {
  const h = createHistogram();
  h.record(50);
  assertEquals(h.countBigInt, 1n);
  assertEquals(h.minBigInt, 50n);
  assertEquals(h.maxBigInt, 50n);
  assertEquals(h.exceedsBigInt, 0n);
});

Deno.test("[perf_hooks]: createHistogram accepts bigint values", () => {
  const h = createHistogram();
  h.record(42n);
  assertEquals(h.count, 1);
  assertEquals(h.min, 42);
});

Deno.test("[perf_hooks]: createHistogram percentile / percentiles", () => {
  const h = createHistogram();
  for (let i = 1; i <= 100; i++) h.record(i);
  assert(h.percentile(50) >= 49 && h.percentile(50) <= 51);
  assertEquals(typeof h.percentileBigInt(50), "bigint");
  const map = h.percentiles;
  assert(map instanceof Map);
  assert(map.size > 0);
  const bigMap = h.percentilesBigInt;
  assert(bigMap instanceof Map);
  for (const v of bigMap.values()) {
    assertEquals(typeof v, "bigint");
  }
});

Deno.test("[perf_hooks]: createHistogram reset", () => {
  const h = createHistogram();
  h.record(10);
  h.record(20);
  assertEquals(h.count, 2);
  h.reset();
  assertEquals(h.count, 0);
});

Deno.test("[perf_hooks]: createHistogram exceeds counter", () => {
  const h = createHistogram({ lowest: 1, highest: 100, figures: 1 });
  h.record(50);
  // 10_000 is above `highest` and should bump exceeds rather than throw.
  h.record(10_000);
  assertEquals(h.exceeds, 1);
  assertEquals(h.exceedsBigInt, 1n);
});

Deno.test("[perf_hooks]: createHistogram add merges another histogram", () => {
  const a = createHistogram();
  const b = createHistogram();
  a.record(10);
  b.record(20);
  b.record(30);
  a.add(b);
  assertEquals(a.count, 3);
});

Deno.test("[perf_hooks]: createHistogram recordDelta", async () => {
  const h = createHistogram();
  h.recordDelta(); // seed
  await new Promise((resolve) => setTimeout(resolve, 5));
  h.recordDelta();
  assertEquals(h.count, 1);
  assert(h.min > 0);
});

Deno.test("[perf_hooks]: createHistogram input validation", () => {
  assertThrows(() => createHistogram({ lowest: 0 }));
  assertThrows(() => createHistogram({ figures: 0 }));
  assertThrows(() => createHistogram({ figures: 6 }));
  assertThrows(() => createHistogram({ lowest: 100, highest: 100 }));
  const h = createHistogram();
  assertThrows(() => h.record(0));
  assertThrows(() => h.record(-1));
  assertThrows(() => h.record("oops" as unknown as number));
});

Deno.test("[perf_hooks]: PerformanceObserver takeRecords", () => {
  const observer = new PerformanceObserver(() => {});
  observer.observe({ entryTypes: ["mark"] });

  performance.mark("take-records-test");

  const records = observer.takeRecords();
  assertEquals(records.length, 1);
  assertEquals(records[0].name, "take-records-test");

  // After takeRecords, buffer should be empty
  const secondRecords = observer.takeRecords();
  assertEquals(secondRecords.length, 0);

  observer.disconnect();
  performance.clearMarks();
});
