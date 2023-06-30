// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import * as perfHooks from "node:perf_hooks";
import { performance } from "node:perf_hooks";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
import {
  assertSpyCall,
  assertSpyCalls,
  spy,
} from "../../../test_util/std/testing/mock.ts";

Deno.test({
  name: "[perf_hooks] performance",
  fn() {
    assertEquals(perfHooks.performance.measure, performance.measure);
    assertEquals(perfHooks.performance.clearMarks, performance.clearMarks);
    assertEquals(perfHooks.performance.mark, performance.mark);
    assertEquals(perfHooks.performance.now, performance.now);
    // @ts-ignore
    assertEquals(perfHooks.performance.toJSON, performance.toJSON);
    perfHooks.performance.measure("test");
    perfHooks.performance.mark("test");
    perfHooks.performance.clearMarks("test");
    perfHooks.performance.now();
    // @ts-ignore
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
    // @ts-ignore
    performance.toJSON();
  },
});

Deno.test({
  name: "[perf_hooks] PerformanceEntry",
  fn() {
    assertEquals<unknown>(perfHooks.PerformanceEntry, PerformanceEntry);
  },
});
