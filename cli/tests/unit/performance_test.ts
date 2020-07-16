// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  unitTest,
  assert,
  assertEquals,
  createResolvable,
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
  const mark = performance.mark("test");
  return new Promise((resolve, reject) => {
    setTimeout(() => {
      try {
        const measure = performance.measure("test", "test");
        assert(measure instanceof PerformanceMeasure);
        assertEquals(measure.detail, null);
        assertEquals(measure.name, "test");
        assertEquals(measure.entryType, "measure");
        assert(measure.startTime > 0);
        assertEquals(mark.startTime, measure.startTime);
        assert(
          measure.duration >= 100,
          `duration below 100ms: ${measure.duration}`,
        );
        assert(
          measure.duration < 500,
          `duration exceeds 500ms: ${measure.duration}`,
        );
        const entries = performance.getEntries();
        assert(entries[entries.length - 1] === measure);
        const measureEntries = performance.getEntriesByName("test", "measure");
        assert(measureEntries[measureEntries.length - 1] === measure);
      } catch (e) {
        return reject(e);
      }
      resolve();
    }, 100);
  });
});
