// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as perfHooks from "node:perf_hooks";
import { performance } from "node:perf_hooks";
import { assertEquals } from ".../../../test_util/std/testing/asserts.ts";
import {
  assertSpyCall,
  assertSpyCalls,
  spy,
} from ".../../../test_util/std/testing/mock.ts";

Deno.test({
  name: "[perf_hooks] performance",
  fn() {
    assertEquals(perfHooks.performance.measure, performance.measure);
    assertEquals(perfHooks.performance.clearMarks, performance.clearMarks);
    assertEquals(perfHooks.performance.mark, performance.mark);
    assertEquals(perfHooks.performance.now, performance.now);
    assertEquals(perfHooks.performance.toJSON, performance.toJSON);
    perfHooks.performance.measure("test");
    perfHooks.performance.mark("test");
    perfHooks.performance.clearMarks("test");
    perfHooks.performance.now();
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
    performance.toJSON();
  },
});

Deno.test({
  name: "[perf_hooks] PerformanceEntry",
  fn() {
    assertEquals<unknown>(perfHooks.PerformanceEntry, PerformanceEntry);
  },
});

Deno.test({
  name: "[perf_hooks] EventTarget methods",
  fn() {
    const handler = spy();
    performance.addEventListener("event", handler);
    assertSpyCalls(handler, 0);
    const e = new Event("event");
    performance.dispatchEvent(e);
    // handler is called once
    assertSpyCalls(handler, 1);
    assertSpyCall(handler, 0, { args: [e] });
    performance.removeEventListener("event", handler);
    performance.dispatchEvent(e);
    // handler is not called anymore
    assertSpyCalls(handler, 1);
  },
});
