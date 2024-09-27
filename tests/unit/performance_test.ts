// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertNotStrictEquals,
  assertStringIncludes,
  assertThrows,
} from "./test_util.ts";

Deno.test({ permissions: {} }, async function performanceNow() {
  const { promise, resolve } = Promise.withResolvers<void>();
  const start = performance.now();
  let totalTime = 0;
  setTimeout(() => {
    const end = performance.now();
    totalTime = end - start;
    resolve();
  }, 10);
  await promise;
  assert(totalTime >= 10);
});

Deno.test(function timeOrigin() {
  const origin = performance.timeOrigin;

  assert(origin > 0);
  assert(Date.now() >= origin);
});

Deno.test(function performanceToJSON() {
  const json = performance.toJSON();

  assert("timeOrigin" in json);
  assert(json.timeOrigin === performance.timeOrigin);
  // check there are no other keys
  assertEquals(Object.keys(json).length, 1);
});

Deno.test(function clearMarks() {
  performance.mark("a");
  performance.mark("a");
  performance.mark("b");
  performance.mark("c");

  const marksNum = performance.getEntriesByType("mark").length;

  performance.clearMarks("a");
  assertEquals(performance.getEntriesByType("mark").length, marksNum - 2);

  performance.clearMarks();
  assertEquals(performance.getEntriesByType("mark").length, 0);
});

Deno.test(function clearMeasures() {
  performance.measure("from-start");
  performance.mark("a");
  performance.measure("from-mark-a", "a");
  performance.measure("from-start");
  performance.measure("from-mark-a", "a");
  performance.mark("b");
  performance.measure("between-a-and-b", "a", "b");

  const measuresNum = performance.getEntriesByType("measure").length;

  performance.clearMeasures("from-start");
  assertEquals(performance.getEntriesByType("measure").length, measuresNum - 2);

  performance.clearMeasures();
  assertEquals(performance.getEntriesByType("measure").length, 0);

  performance.clearMarks();
});

Deno.test(function performanceMark() {
  const mark = performance.mark("test");
  assert(mark instanceof PerformanceMark);
  assertEquals(mark.detail, null);
  assertEquals(mark.name, "test");
  assertEquals(mark.entryType, "mark");
  assert(mark.startTime > 0);
  assertEquals(mark.duration, 0);
  const entries = performance.getEntries();
  assert(entries[entries.length - 1] === mark);
  const markEntries = performance.getEntriesByName("test", "mark");
  assert(markEntries[markEntries.length - 1] === mark);
});

Deno.test(function performanceMarkDetail() {
  const detail = { foo: "foo" };
  const mark = performance.mark("test", { detail });
  assert(mark instanceof PerformanceMark);
  assertEquals(mark.detail, { foo: "foo" });
  assertNotStrictEquals(mark.detail, detail);
});

Deno.test(function performanceMarkDetailArrayBuffer() {
  const detail = new ArrayBuffer(10);
  const mark = performance.mark("test", { detail });
  assert(mark instanceof PerformanceMark);
  assertEquals(mark.detail, new ArrayBuffer(10));
  assertNotStrictEquals(mark.detail, detail);
});

Deno.test(function performanceMarkDetailSubTypedArray() {
  class SubUint8Array extends Uint8Array {}
  const detail = new SubUint8Array([1, 2]);
  const mark = performance.mark("test", { detail });
  assert(mark instanceof PerformanceMark);
  assertEquals(mark.detail, new Uint8Array([1, 2]));
  assertNotStrictEquals(mark.detail, detail);
});

Deno.test(function performanceMeasure() {
  const markName1 = "mark1";
  const measureName1 = "measure1";
  const measureName2 = "measure2";
  const mark1 = performance.mark(markName1);
  // Measure against the inaccurate-but-known-good wall clock
  const now = new Date().valueOf();
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      try {
        const later = new Date().valueOf();
        const measure1 = performance.measure(measureName1, markName1);
        const measure2 = performance.measure(
          measureName2,
          undefined,
          markName1,
        );
        assert(measure1 instanceof PerformanceMeasure);
        assertEquals(measure1.detail, null);
        assertEquals(measure1.name, measureName1);
        assertEquals(measure1.entryType, "measure");
        assert(measure1.startTime > 0);
        assertEquals(measure2.startTime, 0);
        assertEquals(mark1.startTime, measure1.startTime);
        assertEquals(mark1.startTime, measure2.duration);
        assert(
          measure1.duration >= 100,
          `duration below 100ms: ${measure1.duration}`,
        );
        assert(
          measure1.duration < (later - now) * 1.50,
          `duration exceeds 150% of wallclock time: ${measure1.duration}ms vs ${
            later - now
          }ms`,
        );
        const entries = performance.getEntries();
        assert(entries[entries.length - 1] === measure2);
        const entriesByName = performance.getEntriesByName(
          measureName1,
          "measure",
        );
        assert(entriesByName[entriesByName.length - 1] === measure1);
        const measureEntries = performance.getEntriesByType("measure");
        assert(measureEntries[measureEntries.length - 1] === measure2);
      } catch (e) {
        return reject(e);
      }
      resolve();
    }, 100);
  });
});

Deno.test(function performanceMeasureUseMostRecentMark() {
  const markName1 = "mark1";
  const measureName1 = "measure1";
  const mark1 = performance.mark(markName1);
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      try {
        const laterMark1 = performance.mark(markName1);
        const measure1 = performance.measure(measureName1, markName1);
        assertNotEquals(mark1.startTime, measure1.startTime);
        assertEquals(laterMark1.startTime, measure1.startTime);
      } catch (e) {
        return reject(e);
      }
      resolve();
    }, 100);
  });
});

Deno.test(function performanceCustomInspectFunction() {
  assertStringIncludes(Deno.inspect(performance), "Performance");
  assertStringIncludes(
    Deno.inspect(Performance.prototype),
    "Performance",
  );
});

Deno.test(function performanceMarkCustomInspectFunction() {
  const mark1 = performance.mark("mark1");
  assertStringIncludes(Deno.inspect(mark1), "PerformanceMark");
  assertStringIncludes(
    Deno.inspect(PerformanceMark.prototype),
    "PerformanceMark",
  );
});

Deno.test(function performanceMeasureCustomInspectFunction() {
  const measure1 = performance.measure("measure1");
  assertStringIncludes(Deno.inspect(measure1), "PerformanceMeasure");
  assertStringIncludes(
    Deno.inspect(PerformanceMeasure.prototype),
    "PerformanceMeasure",
  );
});

Deno.test(function performanceIllegalConstructor() {
  assertThrows(() => new Performance(), TypeError, "Illegal constructor");
  assertEquals(Performance.length, 0);
});

Deno.test(function performanceEntryIllegalConstructor() {
  assertThrows(() => new PerformanceEntry(), TypeError, "Illegal constructor");
  assertEquals(PerformanceEntry.length, 0);
});

Deno.test(function performanceMeasureIllegalConstructor() {
  assertThrows(
    () => new PerformanceMeasure(),
    TypeError,
    "Illegal constructor",
  );
});

Deno.test(function performanceIsEventTarget() {
  assert(performance instanceof EventTarget);

  return new Promise((resolve) => {
    const handler = () => {
      resolve();
    };

    performance.addEventListener("test", handler, { once: true });
    performance.dispatchEvent(new Event("test"));
  });
});
