const { test } = Deno;
import { bench, runBenchmarks, BenchmarkRunError } from "./bench.ts";
import {
  assertEquals,
  assert,
  assertThrows,
  assertThrowsAsync,
} from "./asserts.ts";

test({
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
          }
        )
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

    assertEquals(benchResult.measured, 5);
    assertEquals(benchResult.filtered, 1);
    assertEquals(benchResult.results.length, 5);

    const resultWithMultipleRunsFiltered = benchResult.results.filter(
      (r) => r.name === "runs100ForIncrementX1e6"
    );
    assertEquals(resultWithMultipleRunsFiltered.length, 1);

    const resultWithMultipleRuns = resultWithMultipleRunsFiltered[0];
    assert(!!resultWithMultipleRuns.runsCount);
    assert(!!resultWithMultipleRuns.runsAvgMs);
    assert(!!resultWithMultipleRuns.runsMs);
    assertEquals(resultWithMultipleRuns.runsCount, 100);
    assertEquals(resultWithMultipleRuns.runsMs!.length, 100);
  },
});

test({
  name: "benchWithoutName",
  fn() {
    assertThrows(
      (): void => {
        bench(() => {});
      },
      Error,
      "The benchmark function must not be anonymous"
    );
  },
});

test({
  name: "benchWithoutStop",
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
      "The benchmark timer's stop method must be called"
    );
  },
});

test({
  name: "benchWithoutStart",
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
      "The benchmark timer's start method must be called"
    );
  },
});

test({
  name: "benchStopBeforeStart",
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
      "The benchmark timer's start method must be called before its stop method"
    );
  },
});
