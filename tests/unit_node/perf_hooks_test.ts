// Copyright 2018-2026 the Deno authors. MIT license.
import * as perfHooks from "node:perf_hooks";
import {
  monitorEventLoopDelay,
  performance,
  PerformanceEntry,
  PerformanceObserver,
} from "node:perf_hooks";
import { assert, assertEquals, assertThrows } from "@std/assert";

Deno.test({
  name: "[perf_hooks] performance",
  fn() {
    assertEquals(perfHooks.performance.measure, performance.measure);
    assertEquals(perfHooks.performance.clearMarks, performance.clearMarks);
    assertEquals(perfHooks.performance.mark, performance.mark);
    assertEquals(perfHooks.performance.now, performance.now);
    assertEquals(
      perfHooks.performance.getEntriesByName,
      performance.getEntriesByName,
    );
    assertEquals(
      perfHooks.performance.getEntriesByType,
      performance.getEntriesByType,
    );
    // @ts-ignore toJSON is not in Performance interface
    assertEquals(perfHooks.performance.toJSON, performance.toJSON);
    perfHooks.performance.measure("test");
    perfHooks.performance.mark("test");
    perfHooks.performance.clearMarks("test");
    perfHooks.performance.now();
    assertEquals(perfHooks.performance.getEntriesByName("event", "mark"), []);
    assertEquals(perfHooks.performance.getEntriesByType("mark"), []);
    // @ts-ignore toJSON is not in Performance interface
    perfHooks.performance.toJSON();
  },
});

Deno.test({
  name: "[perf_hooks] performance destructured",
  fn() {
    performance.measure("test");
    performance.mark("test");
    performance.clearMarks("test");
    performance.now();
    // @ts-ignore toJSON is not in Performance interface
    performance.toJSON();
  },
});

Deno.test({
  name: "[perf_hooks] PerformanceEntry & PerformanceObserver",
  fn() {
    assertEquals<unknown>(perfHooks.PerformanceEntry, PerformanceEntry);
    assertEquals<unknown>(perfHooks.PerformanceObserver, PerformanceObserver);
  },
});

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
