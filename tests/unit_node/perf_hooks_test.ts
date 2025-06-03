// Copyright 2018-2025 the Deno authors. MIT license.
import * as perfHooks from "node:perf_hooks";
import {
  monitorEventLoopDelay,
  performance,
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
