// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  createResolvable,
  unitTest,
} from "./test_util.ts";

unitTest({ perms: { hrtime: false } }, async function performanceNow(): Promise<
  void
> {
  const resolvable = createResolvable();
  const start = performance.now();
  setTimeout((): void => {
    const end = performance.now();
    assert(end - start >= 10);
    resolvable.resolve();
  }, 10);
  await resolvable;
});

unitTest(function performanceMark() {
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

unitTest(function performanceMeasure() {
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

unitTest(function performanceIllegalConstructor() {
  assertThrows(() => new Performance(), TypeError, "Illegal constructor.");
  assertEquals(Performance.length, 0);
});

unitTest(function performanceEntryIllegalConstructor() {
  assertThrows(() => new PerformanceEntry(), TypeError, "Illegal constructor.");
  assertEquals(PerformanceEntry.length, 0);
});

unitTest(function performanceMeasureIllegalConstructor() {
  assertThrows(
    () => new PerformanceMeasure(),
    TypeError,
    "Illegal constructor.",
  );
});
