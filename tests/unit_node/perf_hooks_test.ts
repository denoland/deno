// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import * as perfHooks from "node:perf_hooks";
import { performance, PerformanceObserver } from "node:perf_hooks";
import { assertEquals, assertThrows } from "@std/assert/mod.ts";

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
