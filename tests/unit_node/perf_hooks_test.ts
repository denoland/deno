// Copyright 2018-2026 the Deno authors. MIT license.
import {
  monitorEventLoopDelay,
  performance,
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

Deno.test({
  name: "[perf_hooks] PerformanceObserver observes measure entries",
  async fn() {
    let observerCalled = false;
    let observedEntries: PerformanceEntry[] = [];

    const observer = new PerformanceObserver((list) => {
      observerCalled = true;
      observedEntries = observedEntries.concat(list.getEntries());
    });

    observer.observe({ entryTypes: ["measure"] });

    // Create marks and measure
    performance.mark("test-start");
    performance.mark("test-end");
    performance.measure("test-measure", "test-start", "test-end");

    // Wait for microtask queue to flush
    await new Promise((resolve) => setTimeout(resolve, 10));

    observer.disconnect();

    assert(observerCalled, "Observer callback should be triggered");
    assert(observedEntries.length > 0, "Should have observed entries");
    const measureEntry = observedEntries.find((e) => e.name === "test-measure");
    assert(measureEntry, "Should observe the test-measure entry");
    assertEquals(measureEntry!.entryType, "measure");

    // Cleanup
    performance.clearMarks("test-start");
    performance.clearMarks("test-end");
    performance.clearMeasures("test-measure");
  },
});

Deno.test({
  name: "[perf_hooks] PerformanceObserver observes mark entries",
  async fn() {
    let observedEntries: PerformanceEntry[] = [];

    const observer = new PerformanceObserver((list) => {
      observedEntries = observedEntries.concat(list.getEntries());
    });

    observer.observe({ entryTypes: ["mark"] });

    performance.mark("observed-mark");

    // Wait for microtask queue to flush
    await new Promise((resolve) => setTimeout(resolve, 10));

    observer.disconnect();

    assert(observedEntries.length > 0, "Should have observed mark entries");
    const markEntry = observedEntries.find((e) => e.name === "observed-mark");
    assert(markEntry, "Should observe the observed-mark entry");
    assertEquals(markEntry!.entryType, "mark");

    // Cleanup
    performance.clearMarks("observed-mark");
  },
});

Deno.test({
  name: "[perf_hooks] PerformanceObserver.supportedEntryTypes",
  fn() {
    assertEquals(PerformanceObserver.supportedEntryTypes, ["mark", "measure"]);
  },
});

Deno.test({
  name: "[perf_hooks] PerformanceObserver disconnect stops observations",
  async fn() {
    let callCount = 0;

    const observer = new PerformanceObserver(() => {
      callCount++;
    });

    observer.observe({ entryTypes: ["mark"] });

    performance.mark("before-disconnect");
    await new Promise((resolve) => setTimeout(resolve, 10));

    observer.disconnect();

    performance.mark("after-disconnect");
    await new Promise((resolve) => setTimeout(resolve, 10));

    assertEquals(
      callCount,
      1,
      "Observer should only be called once before disconnect",
    );

    // Cleanup
    performance.clearMarks("before-disconnect");
    performance.clearMarks("after-disconnect");
  },
});

Deno.test({
  name: "[perf_hooks] PerformanceObserver takeRecords",
  fn() {
    const observer = new PerformanceObserver(() => {});
    observer.observe({ entryTypes: ["mark"] });

    // takeRecords returns buffered entries
    const records = observer.takeRecords();
    assertEquals(Array.isArray(records), true);

    observer.disconnect();
  },
});

Deno.test({
  name: "[perf_hooks] PerformanceObserver requires callback function",
  fn() {
    assertThrows(
      () => {
        // @ts-expect-error testing invalid input
        new PerformanceObserver("not a function");
      },
      TypeError,
      "callback must be a function",
    );
  },
});

Deno.test({
  name: "[perf_hooks] PerformanceObserver observe requires entryTypes or type",
  fn() {
    const observer = new PerformanceObserver(() => {});
    assertThrows(
      () => {
        observer.observe({});
      },
      TypeError,
      "observe requires either 'entryTypes' or 'type' option",
    );
  },
});
