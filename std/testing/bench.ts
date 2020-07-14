// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { deepAssign } from "../_util/deep_assign.ts";

interface BenchmarkClock {
  start: number;
  stop: number;
  for?: string;
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
  /** Defines how many times the provided `func` should be benchmarked in succession */
  runs?: number;
}

/** Defines runBenchmark's run constraints by matching benchmark names. */
export interface BenchmarkRunOptions {
  /** Only benchmarks which name match this regexp will be run*/
  only?: RegExp;
  /** Benchmarks which name match this regexp will be skipped */
  skip?: RegExp;
  /** Setting it to true prevents default benchmarking progress logs to the commandline*/
  silent?: boolean;
}

/** Defines clearBenchmark's constraints by matching benchmark names. */
export interface BenchmarkClearOptions {
  /** Only benchmarks which name match this regexp will be removed */
  only?: RegExp;
  /** Benchmarks which name match this regexp will be kept */
  skip?: RegExp;
}

/** Defines the result of a single benchmark */
export interface BenchmarkResult {
  /** The name of the benchmark */
  name: string;
  /** The total time it took to run a given bechmark  */
  totalMs: number;
  /** Times the benchmark was run in succession. */
  runsCount: number;
  /** The average time of running the benchmark in milliseconds. */
  measuredRunsAvgMs: number;
  /** The individual measurements in milliseconds it took to run the benchmark.*/
  measuredRunsMs: number[];
}

/** Defines the result of a `runBenchmarks` call */
export interface BenchmarkRunResult {
  /** How many benchmark were ignored by the provided `only` and `skip` */
  filtered: number;
  /** The individual results for each benchmark that was run */
  results: BenchmarkResult[];
}

/** Defines the current progress during the run of `runBenchmarks` */
export interface BenchmarkRunProgress extends BenchmarkRunResult {
  /** List of the queued benchmarks to run with their name and their run count */
  queued: Array<{ name: string; runsCount: number }>;
  /** The currently running benchmark with its name, run count and the already finished measurements in milliseconds */
  running?: { name: string; runsCount: number; measuredRunsMs: number[] };
  /** Indicates in which state benchmarking currently is */
  state: ProgressState;
}

/** Defines the states `BenchmarkRunProgress` can be in */
export enum ProgressState {
  BenchmarkingStart = "benchmarking_start",
  BenchStart = "bench_start",
  BenchPartialResult = "bench_partial_result",
  BenchResult = "bench_result",
  BenchmarkingEnd = "benchmarking_end",
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
  return Deno.noColor ? text : `\x1b[31m${text}\x1b[0m`;
}

function blue(text: string): string {
  return Deno.noColor ? text : `\x1b[34m${text}\x1b[0m`;
}

function verifyOr1Run(runs?: number): number {
  return runs && runs >= 1 && runs !== Infinity ? Math.floor(runs) : 1;
}

function assertTiming(clock: BenchmarkClock): void {
  // NaN indicates that a benchmark has not been timed properly
  if (!clock.stop) {
    throw new BenchmarkRunError(
      `Running benchmarks FAILED during benchmark named [${clock.for}]. The benchmark timer's stop method must be called`,
      clock.for,
    );
  } else if (!clock.start) {
    throw new BenchmarkRunError(
      `Running benchmarks FAILED during benchmark named [${clock.for}]. The benchmark timer's start method must be called`,
      clock.for,
    );
  } else if (clock.start > clock.stop) {
    throw new BenchmarkRunError(
      `Running benchmarks FAILED during benchmark named [${clock.for}]. The benchmark timer's start method must be called before its stop method`,
      clock.for,
    );
  }
}

function createBenchmarkTimer(clock: BenchmarkClock): BenchmarkTimer {
  return {
    start(): void {
      clock.start = performance.now();
    },
    stop(): void {
      if (isNaN(clock.start)) {
        throw new BenchmarkRunError(
          `Running benchmarks FAILED during benchmark named [${clock.for}]. The benchmark timer's start method must be called before its stop method`,
          clock.for,
        );
      }
      clock.stop = performance.now();
    },
  };
}

const candidates: BenchmarkDefinition[] = [];

/** Registers a benchmark as a candidate for the runBenchmarks executor. */
export function bench(
  benchmark: BenchmarkDefinition | BenchmarkFunction,
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

/** Clears benchmark candidates which name matches `only` and doesn't match `skip`.
 * Removes all candidates if options were not provided */
export function clearBenchmarks({
  only = /[^\s]/,
  skip = /$^/,
}: BenchmarkClearOptions = {}): void {
  const keep = candidates.filter(
    ({ name }): boolean => !only.test(name) || skip.test(name),
  );
  candidates.splice(0, candidates.length);
  candidates.push(...keep);
}

/**
 * Runs all registered and non-skipped benchmarks serially.
 *
 * @param [progressCb] provides the possibility to get updates of the current progress during the run of the benchmarking
 * @returns results of the benchmarking
 */
export async function runBenchmarks(
  { only = /[^\s]/, skip = /^\s*$/, silent }: BenchmarkRunOptions = {},
  progressCb?: (progress: BenchmarkRunProgress) => void | Promise<void>,
): Promise<BenchmarkRunResult> {
  // Filtering candidates by the "only" and "skip" constraint
  const benchmarks: BenchmarkDefinition[] = candidates.filter(
    ({ name }): boolean => only.test(name) && !skip.test(name),
  );
  // Init main counters and error flag
  const filtered = candidates.length - benchmarks.length;
  let failError: Error | undefined = undefined;
  // Setting up a shared benchmark clock and timer
  const clock: BenchmarkClock = { start: NaN, stop: NaN };
  const b = createBenchmarkTimer(clock);

  // Init progress data
  const progress: BenchmarkRunProgress = {
    // bench.run is already ensured with verifyOr1Run on register
    queued: benchmarks.map((bench) => ({
      name: bench.name,
      runsCount: bench.runs!,
    })),
    results: [],
    filtered,
    state: ProgressState.BenchmarkingStart,
  };

  // Publish initial progress data
  await publishProgress(progress, ProgressState.BenchmarkingStart, progressCb);

  if (!silent) {
    console.log(
      "running",
      benchmarks.length,
      `benchmark${benchmarks.length === 1 ? " ..." : "s ..."}`,
    );
  }

  // Iterating given benchmark definitions (await-in-loop)
  for (const { name, runs = 0, func } of benchmarks) {
    if (!silent) {
      // See https://github.com/denoland/deno/pull/1452 about groupCollapsed
      console.groupCollapsed(`benchmark ${name} ... `);
    }

    // Provide the benchmark name for clock assertions
    clock.for = name;

    // Remove benchmark from queued
    const queueIndex = progress.queued.findIndex(
      (queued) => queued.name === name && queued.runsCount === runs,
    );
    if (queueIndex != -1) {
      progress.queued.splice(queueIndex, 1);
    }
    // Init the progress of the running benchmark
    progress.running = { name, runsCount: runs, measuredRunsMs: [] };
    // Publish starting of a benchmark
    await publishProgress(progress, ProgressState.BenchStart, progressCb);

    // Trying benchmark.func
    let result = "";
    try {
      // Averaging runs
      let pendingRuns = runs;
      let totalMs = 0;

      // Would be better 2 not run these serially
      while (true) {
        // b is a benchmark timer interfacing an unset (NaN) benchmark clock
        await func(b);
        // Making sure the benchmark was started/stopped properly
        assertTiming(clock);

        // Calculate length of run
        const measuredMs = clock.stop - clock.start;

        // Summing up
        totalMs += measuredMs;
        // Adding partial result
        progress.running.measuredRunsMs.push(measuredMs);
        // Publish partial benchmark results
        await publishProgress(
          progress,
          ProgressState.BenchPartialResult,
          progressCb,
        );

        // Resetting the benchmark clock
        clock.start = clock.stop = NaN;
        // Once all ran
        if (!--pendingRuns) {
          result = runs == 1
            ? `${totalMs}ms`
            : `${runs} runs avg: ${totalMs / runs}ms`;
          // Adding results
          progress.results.push({
            name,
            totalMs,
            runsCount: runs,
            measuredRunsAvgMs: totalMs / runs,
            measuredRunsMs: progress.running.measuredRunsMs,
          });
          // Clear currently running
          delete progress.running;
          // Publish results of the benchmark
          await publishProgress(
            progress,
            ProgressState.BenchResult,
            progressCb,
          );
          break;
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

    // Resetting the benchmark clock
    clock.start = clock.stop = NaN;
    delete clock.for;
  }

  // Indicate finished running
  delete progress.queued;
  // Publish final result in Cb too
  await publishProgress(progress, ProgressState.BenchmarkingEnd, progressCb);

  if (!silent) {
    // Closing results
    console.log(
      `benchmark result: ${failError ? red("FAIL") : blue("DONE")}. ` +
        `${progress.results.length} measured; ${filtered} filtered`,
    );
  }

  // Throw error if there was a failing benchmark
  if (failError) {
    throw failError;
  }

  const benchmarkRunResult = {
    filtered,
    results: progress.results,
  };

  return benchmarkRunResult;
}

async function publishProgress(
  progress: BenchmarkRunProgress,
  state: ProgressState,
  progressCb?: (progress: BenchmarkRunProgress) => void | Promise<void>,
): Promise<void> {
  progressCb && (await progressCb(cloneProgressWithState(progress, state)));
}

function cloneProgressWithState(
  progress: BenchmarkRunProgress,
  state: ProgressState,
): BenchmarkRunProgress {
  return deepAssign({}, progress, { state }) as BenchmarkRunProgress;
}
