import {
  bench,
  runBenchmarks,
  BenchmarkRunError,
  clearBenchmarks,
  BenchmarkRunProgress,
  ProgressState,
} from "./bench.ts";
import {
  assertEquals,
  assert,
  assertThrows,
  assertThrowsAsync,
} from "./asserts.ts";

Deno.test({
  name: "benching",

  fn: async function (): Promise<void> {
    bench(function forIncrementX1e9(b): void {
      b.start();
      for (let i = 0; i < 1e9; i++);
      b.stop();
    });

    bench(function forDecrementX1e9(b): void {
      b.start();
      for (let i = 1e9; i > 0; i--);
      b.stop();
    });

    bench(async function forAwaitFetchDenolandX10(b): Promise<void> {
      b.start();
      for (let i = 0; i < 10; i++) {
        const r = await fetch("https://deno.land/");
        await r.text();
      }
      b.stop();
    });

    bench(async function promiseAllFetchDenolandX10(b): Promise<void> {
      const urls = new Array(10).fill("https://deno.land/");
      b.start();
      await Promise.all(
        urls.map(
          async (denoland: string): Promise<void> => {
            const r = await fetch(denoland);
            await r.text();
          },
        ),
      );
      b.stop();
    });

    bench({
      name: "runs100ForIncrementX1e6",
      runs: 100,
      func(b): void {
        b.start();
        for (let i = 0; i < 1e6; i++);
        b.stop();
      },
    });

    bench(function throwing(b): void {
      b.start();
      // Throws bc the timer's stop method is never called
    });

    const benchResult = await runBenchmarks({ skip: /throw/ });

    assertEquals(benchResult.filtered, 1);
    assertEquals(benchResult.results.length, 5);

    const resultWithSingleRunsFiltered = benchResult.results.filter(
      ({ name }) => name === "forDecrementX1e9",
    );
    assertEquals(resultWithSingleRunsFiltered.length, 1);

    const resultWithSingleRuns = resultWithSingleRunsFiltered[0];
    assert(!!resultWithSingleRuns.runsCount);
    assert(!!resultWithSingleRuns.measuredRunsAvgMs);
    assert(!!resultWithSingleRuns.measuredRunsMs);
    assertEquals(resultWithSingleRuns.runsCount, 1);
    assertEquals(resultWithSingleRuns.measuredRunsMs.length, 1);

    const resultWithMultipleRunsFiltered = benchResult.results.filter(
      ({ name }) => name === "runs100ForIncrementX1e6",
    );
    assertEquals(resultWithMultipleRunsFiltered.length, 1);

    const resultWithMultipleRuns = resultWithMultipleRunsFiltered[0];
    assert(!!resultWithMultipleRuns.runsCount);
    assert(!!resultWithMultipleRuns.measuredRunsAvgMs);
    assert(!!resultWithMultipleRuns.measuredRunsMs);
    assertEquals(resultWithMultipleRuns.runsCount, 100);
    assertEquals(resultWithMultipleRuns.measuredRunsMs.length, 100);

    clearBenchmarks();
  },
});

Deno.test({
  name: "Bench without name should throw",
  fn() {
    assertThrows(
      (): void => {
        bench(() => {});
      },
      Error,
      "The benchmark function must not be anonymous",
    );
  },
});

Deno.test({
  name: "Bench without stop should throw",
  fn: async function (): Promise<void> {
    await assertThrowsAsync(
      async (): Promise<void> => {
        bench(function benchWithoutStop(b): void {
          b.start();
          // Throws bc the timer's stop method is never called
        });
        await runBenchmarks({ only: /benchWithoutStop/, silent: true });
      },
      BenchmarkRunError,
      "The benchmark timer's stop method must be called",
    );
  },
});

Deno.test({
  name: "Bench without start should throw",
  fn: async function (): Promise<void> {
    await assertThrowsAsync(
      async (): Promise<void> => {
        bench(function benchWithoutStart(b): void {
          b.stop();
          // Throws bc the timer's start method is never called
        });
        await runBenchmarks({ only: /benchWithoutStart/, silent: true });
      },
      BenchmarkRunError,
      "The benchmark timer's start method must be called",
    );
  },
});

Deno.test({
  name: "Bench with stop before start should throw",
  fn: async function (): Promise<void> {
    await assertThrowsAsync(
      async (): Promise<void> => {
        bench(function benchStopBeforeStart(b): void {
          b.stop();
          b.start();
          // Throws bc the timer's stop is called before start
        });
        await runBenchmarks({ only: /benchStopBeforeStart/, silent: true });
      },
      BenchmarkRunError,
      "The benchmark timer's start method must be called before its stop method",
    );
  },
});

Deno.test({
  name: "clearBenchmarks should clear all candidates",
  fn: async function (): Promise<void> {
    dummyBench("test");

    clearBenchmarks();
    const benchingResults = await runBenchmarks({ silent: true });

    assertEquals(benchingResults.filtered, 0);
    assertEquals(benchingResults.results.length, 0);
  },
});

Deno.test({
  name: "clearBenchmarks with only as option",
  fn: async function (): Promise<void> {
    // to reset candidates
    clearBenchmarks();

    dummyBench("test");
    dummyBench("onlyclear");

    clearBenchmarks({ only: /only/ });
    const benchingResults = await runBenchmarks({ silent: true });

    assertEquals(benchingResults.filtered, 0);
    assertEquals(benchingResults.results.length, 1);
    assertEquals(benchingResults.results[0].name, "test");
  },
});

Deno.test({
  name: "clearBenchmarks with skip as option",
  fn: async function (): Promise<void> {
    // to reset candidates
    clearBenchmarks();

    dummyBench("test");
    dummyBench("skipclear");

    clearBenchmarks({ skip: /skip/ });
    const benchingResults = await runBenchmarks({ silent: true });

    assertEquals(benchingResults.filtered, 0);
    assertEquals(benchingResults.results.length, 1);
    assertEquals(benchingResults.results[0].name, "skipclear");
  },
});

Deno.test({
  name: "clearBenchmarks with only and skip as option",
  fn: async function (): Promise<void> {
    // to reset candidates
    clearBenchmarks();

    dummyBench("test");
    dummyBench("clearonly");
    dummyBench("clearskip");
    dummyBench("clearonly");

    clearBenchmarks({ only: /clear/, skip: /skip/ });
    const benchingResults = await runBenchmarks({ silent: true });

    assertEquals(benchingResults.filtered, 0);
    assertEquals(benchingResults.results.length, 2);
    assert(!!benchingResults.results.find(({ name }) => name === "test"));
    assert(!!benchingResults.results.find(({ name }) => name === "clearskip"));
  },
});

Deno.test({
  name: "progressCallback of runBenchmarks",
  fn: async function (): Promise<void> {
    clearBenchmarks();
    dummyBench("skip");
    dummyBench("single");
    dummyBench("multiple", 2);

    const progressCallbacks: BenchmarkRunProgress[] = [];

    const benchingResults = await runBenchmarks(
      { skip: /skip/, silent: true },
      (progress) => {
        progressCallbacks.push(progress);
      },
    );

    let pc = 0;
    // Assert initial progress before running
    let progress = progressCallbacks[pc++];
    assertEquals(progress.state, ProgressState.BenchmarkingStart);
    assertEquals(progress.filtered, 1);
    assertEquals(progress.queued.length, 2);
    assertEquals(progress.running, undefined);
    assertEquals(progress.results, []);

    // Assert start of bench "single"
    progress = progressCallbacks[pc++];
    assertEquals(progress.state, ProgressState.BenchStart);
    assertEquals(progress.filtered, 1);
    assertEquals(progress.queued.length, 1);
    assert(!!progress.queued.find(({ name }) => name == "multiple"));
    assertEquals(progress.running, {
      name: "single",
      runsCount: 1,
      measuredRunsMs: [],
    });
    assertEquals(progress.results, []);

    // Assert running result of bench "single"
    progress = progressCallbacks[pc++];
    assertEquals(progress.state, ProgressState.BenchPartialResult);
    assertEquals(progress.queued.length, 1);
    assertEquals(progress.running!.measuredRunsMs.length, 1);
    assertEquals(progress.results.length, 0);

    // Assert result of bench "single"
    progress = progressCallbacks[pc++];
    assertEquals(progress.state, ProgressState.BenchResult);
    assertEquals(progress.queued.length, 1);
    assertEquals(progress.running, undefined);
    assertEquals(progress.results.length, 1);
    assert(!!progress.results.find(({ name }) => name == "single"));

    // Assert start of bench "multiple"
    progress = progressCallbacks[pc++];
    assertEquals(progress.state, ProgressState.BenchStart);
    assertEquals(progress.queued.length, 0);
    assertEquals(progress.running, {
      name: "multiple",
      runsCount: 2,
      measuredRunsMs: [],
    });
    assertEquals(progress.results.length, 1);

    // Assert first result of bench "multiple"
    progress = progressCallbacks[pc++];
    assertEquals(progress.state, ProgressState.BenchPartialResult);
    assertEquals(progress.queued.length, 0);
    assertEquals(progress.running!.measuredRunsMs.length, 1);
    assertEquals(progress.results.length, 1);

    // Assert second result of bench "multiple"
    progress = progressCallbacks[pc++];
    assertEquals(progress.state, ProgressState.BenchPartialResult);
    assertEquals(progress.queued.length, 0);
    assertEquals(progress.running!.measuredRunsMs.length, 2);
    assertEquals(progress.results.length, 1);

    // Assert finish of bench "multiple"
    progress = progressCallbacks[pc++];
    assertEquals(progress.state, ProgressState.BenchResult);
    assertEquals(progress.queued.length, 0);
    assertEquals(progress.running, undefined);
    assertEquals(progress.results.length, 2);
    assert(!!progress.results.find(({ name }) => name == "single"));
    const resultOfMultiple = progress.results.filter(
      ({ name }) => name == "multiple",
    );
    assertEquals(resultOfMultiple.length, 1);
    assert(!!resultOfMultiple[0].measuredRunsMs);
    assert(!isNaN(resultOfMultiple[0].measuredRunsAvgMs));
    assertEquals(resultOfMultiple[0].measuredRunsMs.length, 2);

    // The last progress should equal the final result from promise except the state property
    progress = progressCallbacks[pc++];
    assertEquals(progress.state, ProgressState.BenchmarkingEnd);
    delete progress.state;
    assertEquals(progress, benchingResults);
  },
});

Deno.test({
  name: "async progressCallback",
  fn: async function (): Promise<void> {
    clearBenchmarks();
    dummyBench("single");

    const asyncCallbacks = [];

    await runBenchmarks({ silent: true }, (progress) => {
      return new Promise((resolve) => {
        queueMicrotask(() => {
          asyncCallbacks.push(progress);
          resolve();
        });
      });
    });

    assertEquals(asyncCallbacks.length, 5);
  },
});

function dummyBench(name: string, runs = 1): void {
  bench({
    name,
    runs,
    func(b) {
      b.start();
      b.stop();
    },
  });
}
