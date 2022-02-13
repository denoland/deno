// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
  assertThrows,
  deferred,
} from "./test_util.ts";

Deno.test({ permissions: { hrtime: false } }, async function performanceNow() {
  const resolvable = deferred();
  const start = performance.now();
  let totalTime = 0;
  setTimeout(() => {
    const end = performance.now();
    totalTime = end - start;
    resolvable.resolve();
  }, 10);
  await resolvable;
  assert(totalTime >= 10);
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

Deno.test(function performanceMeasure() {
  const markName1 = "mark1";
  const measureName1 = "measure1";
  const measureName2 = "measure2";
  const mark1 = performance.mark(markName1);
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      try {
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
          measure1.duration < 500,
          `duration exceeds 500ms: ${measure1.duration}`,
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
