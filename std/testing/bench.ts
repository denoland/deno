// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const { noColor } = Deno;

interface BenchmarkClock {
  start: number;
  stop: number;
}

/** Provides methods for starting and stopping a benchmark clock. */
export interface BenchmarkTimer {
  start: () => void;
  stop: () => void;
}

/** Defines a benchmark through a named function. */
export interface BenchmarkFunction {
  (b: BenchmarkTimer): void | Promise<void>;
  name: string;
}

/** Defines a benchmark definition with configurable runs. */
export interface BenchmarkDefinition {
  func: BenchmarkFunction;
  name: string;
  runs?: number;
}

/** Defines runBenchmark's run constraints by matching benchmark names. */
export interface BenchmarkRunOptions {
  only?: RegExp;
  skip?: RegExp;
  silent?: boolean;
}

export interface BenchmarkResult {
  name: string;
  totalMs: number;
  runsCount?: number;
  runsAvgMs?: number;
  runsMs?: number[];
}

export interface BenchmarkRunResult {
  measured: number;
  filtered: number;
  results: BenchmarkResult[];
}

export class BenchmarkRunError extends Error {
  benchmarkName?: string;
  constructor(msg: string, benchmarkName?: string) {
    super(msg);
    this.name = "BenchmarkRunError";
    this.benchmarkName = benchmarkName;
  }
}

function red(text: string): string {
  return noColor ? text : `\x1b[31m${text}\x1b[0m`;
}

function blue(text: string): string {
  return noColor ? text : `\x1b[34m${text}\x1b[0m`;
}

function verifyOr1Run(runs?: number): number {
  return runs && runs >= 1 && runs !== Infinity ? Math.floor(runs) : 1;
}

function assertTiming(clock: BenchmarkClock, benchmarkName: string): void {
  // NaN indicates that a benchmark has not been timed properly
  if (!clock.stop) {
    throw new BenchmarkRunError(
      `Running benchmarks FAILED during benchmark named [${benchmarkName}]. The benchmark timer's stop method must be called`,
      benchmarkName
    );
  } else if (!clock.start) {
    throw new BenchmarkRunError(
      `Running benchmarks FAILED during benchmark named [${benchmarkName}]. The benchmark timer's start method must be called`,
      benchmarkName
    );
  } else if (clock.start > clock.stop) {
    throw new BenchmarkRunError(
      `Running benchmarks FAILED during benchmark named [${benchmarkName}]. The benchmark timer's start method must be called before its stop method`,
      benchmarkName
    );
  }
}

function createBenchmarkTimer(clock: BenchmarkClock): BenchmarkTimer {
  return {
    start(): void {
      clock.start = performance.now();
    },
    stop(): void {
      clock.stop = performance.now();
    },
  };
}

const candidates: BenchmarkDefinition[] = [];

/** Registers a benchmark as a candidate for the runBenchmarks executor. */
export function bench(
  benchmark: BenchmarkDefinition | BenchmarkFunction
): void {
  if (!benchmark.name) {
    throw new Error("The benchmark function must not be anonymous");
  }
  if (typeof benchmark === "function") {
    candidates.push({ name: benchmark.name, runs: 1, func: benchmark });
  } else {
    candidates.push({
      name: benchmark.name,
      runs: verifyOr1Run(benchmark.runs),
      func: benchmark.func,
    });
  }
}

/** Runs all registered and non-skipped benchmarks serially. */
export async function runBenchmarks({
  only = /[^\s]/,
  skip = /^\s*$/,
  silent,
}: BenchmarkRunOptions = {}): Promise<BenchmarkRunResult> {
  // Filtering candidates by the "only" and "skip" constraint
  const benchmarks: BenchmarkDefinition[] = candidates.filter(
    ({ name }): boolean => only.test(name) && !skip.test(name)
  );
  // Init main counters and error flag
  const filtered = candidates.length - benchmarks.length;
  let measured = 0;
  let failError: Error | undefined = undefined;
  // Setting up a shared benchmark clock and timer
  const clock: BenchmarkClock = { start: NaN, stop: NaN };
  const b = createBenchmarkTimer(clock);

  if (!silent) {
    // Iterating given benchmark definitions (await-in-loop)
    console.log(
      "running",
      benchmarks.length,
      `benchmark${benchmarks.length === 1 ? " ..." : "s ..."}`
    );
  }

  // Initializing results array
  const benchmarkResults: BenchmarkResult[] = [];
  for (const { name, runs = 0, func } of benchmarks) {
    if (!silent) {
      // See https://github.com/denoland/deno/pull/1452 about groupCollapsed
      console.groupCollapsed(`benchmark ${name} ... `);
    }

    // Trying benchmark.func
    let result = "";
    try {
      if (runs === 1) {
        // b is a benchmark timer interfacing an unset (NaN) benchmark clock
        await func(b);
        // Making sure the benchmark was started/stopped properly
        assertTiming(clock, name);
        result = `${clock.stop - clock.start}ms`;
        // Adding one-time run to results
        benchmarkResults.push({ name, totalMs: clock.stop - clock.start });
      } else if (runs > 1) {
        // Averaging runs
        let pendingRuns = runs;
        let totalMs = 0;
        // Initializing array holding individual runs ms
        const runsMs = [];
        // Would be better 2 not run these serially
        while (true) {
          // b is a benchmark timer interfacing an unset (NaN) benchmark clock
          await func(b);
          // Making sure the benchmark was started/stopped properly
          assertTiming(clock, name);
          // Summing up
          totalMs += clock.stop - clock.start;
          // Adding partial result
          runsMs.push(clock.stop - clock.start);
          // Resetting the benchmark clock
          clock.start = clock.stop = NaN;
          // Once all ran
          if (!--pendingRuns) {
            result = `${runs} runs avg: ${totalMs / runs}ms`;
            // Adding result of multiple runs
            benchmarkResults.push({
              name,
              totalMs,
              runsCount: runs,
              runsAvgMs: totalMs / runs,
              runsMs,
            });
            break;
          }
        }
      }
    } catch (err) {
      failError = err;

      if (!silent) {
        console.groupEnd();
        console.error(red(err.stack));
      }

      break;
    }

    if (!silent) {
      // Reporting
      console.log(blue(result));
      console.groupEnd();
    }

    measured++;
    // Resetting the benchmark clock
    clock.start = clock.stop = NaN;
  }

  if (!silent) {
    // Closing results
    console.log(
      `benchmark result: ${!!failError ? red("FAIL") : blue("DONE")}. ` +
        `${measured} measured; ${filtered} filtered`
    );
  }

  // Making sure the program exit code is not zero in case of failure
  if (!!failError) {
    throw failError;
  }

  const benchmarkRunResult = {
    measured,
    filtered,
    results: benchmarkResults,
  };

  return benchmarkRunResult;
}
