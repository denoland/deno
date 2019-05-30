// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { green, red } from "../colors/mod.ts";

export type TestFunction = () => void | Promise<void>;

export interface TestDefinition {
  fn: TestFunction;
  name: string;
}

let filterRegExp: RegExp | null;
const candidates: TestDefinition[] = [];

let filtered = 0;

// Must be called before any test() that needs to be filtered.
export function setFilter(s: string): void {
  filterRegExp = new RegExp(s, "i");
}

function filter(name: string): boolean {
  if (filterRegExp) {
    return filterRegExp.test(name);
  } else {
    return true;
  }
}

export function test(t: TestDefinition | TestFunction): void {
  const fn: TestFunction = typeof t === "function" ? t : t.fn;
  const name: string = t.name;

  if (!name) {
    throw new Error("Test function may not be anonymous");
  }
  if (filter(name)) {
    candidates.push({ fn, name });
  } else {
    filtered++;
  }
}

const RED_FAILED = red("FAILED");
const GREEN_OK = green("ok");

interface TestStats {
  filtered: number;
  ignored: number;
  measured: number;
  passed: number;
  failed: number;
}

interface TestResult {
  name: string;
  error?: Error;
  ok: boolean;
  printed: boolean;
}

interface TestResults {
  keys: Map<string, number>;
  cases: Map<number, TestResult>;
}

function createTestResults(tests: TestDefinition[]): TestResults {
  return tests.reduce(
    (acc: TestResults, { name }: TestDefinition, i: number): TestResults => {
      acc.keys.set(name, i);
      acc.cases.set(i, { name, printed: false, ok: false, error: undefined });
      return acc;
    },
    { cases: new Map(), keys: new Map() }
  );
}

function report(result: TestResult): void {
  if (result.ok) {
    console.log(`test ${result.name} ... ${GREEN_OK}`);
  } else if (result.error) {
    console.error(
      `test ${result.name} ... ${RED_FAILED}\n${result.error.stack}`
    );
  } else {
    console.log(`test ${result.name} ... unresolved`);
  }
  result.printed = true;
}

function printResults(
  stats: TestStats,
  results: TestResults,
  flush: boolean,
  exitOnFail: boolean
): void {
  if (flush) {
    for (const result of results.cases.values()) {
      if (!result.printed) {
        report(result);
        if (result.error && exitOnFail) {
          break;
        }
      }
    }
  }
  // Attempting to match the output of Rust's test runner.
  console.log(
    `\ntest result: ${stats.failed ? RED_FAILED : GREEN_OK}. ` +
      `${stats.passed} passed; ${stats.failed} failed; ` +
      `${stats.ignored} ignored; ${stats.measured} measured; ` +
      `${stats.filtered} filtered out\n`
  );
}

function previousPrinted(name: string, results: TestResults): boolean {
  const curIndex: number = results.keys.get(name)!;
  if (curIndex === 0) {
    return true;
  }
  return results.cases.get(curIndex - 1)!.printed;
}

async function createTestCase(
  stats: TestStats,
  results: TestResults,
  exitOnFail: boolean,
  { fn, name }: TestDefinition
): Promise<void> {
  const result: TestResult = results.cases.get(results.keys.get(name)!)!;
  try {
    await fn();
    stats.passed++;
    result.ok = true;
  } catch (err) {
    stats.failed++;
    result.error = err;
    if (exitOnFail) {
      throw err;
    }
  }
  if (previousPrinted(name, results)) {
    report(result);
  }
}

function initTestCases(
  stats: TestStats,
  results: TestResults,
  tests: TestDefinition[],
  exitOnFail: boolean
): Array<Promise<void>> {
  return tests.map(createTestCase.bind(null, stats, results, exitOnFail));
}

async function runTestsParallel(
  stats: TestStats,
  results: TestResults,
  tests: TestDefinition[],
  exitOnFail: boolean
): Promise<void> {
  try {
    await Promise.all(initTestCases(stats, results, tests, exitOnFail));
  } catch (_) {
    // The error was thrown to stop awaiting all promises if exitOnFail === true
    // stats.failed has been incremented and the error stored in results
  }
}

async function runTestsSerial(
  stats: TestStats,
  tests: TestDefinition[],
  exitOnFail: boolean
): Promise<void> {
  for (const { fn, name } of tests) {
    // See https://github.com/denoland/deno/pull/1452
    // about this usage of groupCollapsed
    console.groupCollapsed(`test ${name} `);
    try {
      await fn();
      stats.passed++;
      console.log("...", GREEN_OK);
      console.groupEnd();
    } catch (err) {
      console.log("...", RED_FAILED);
      console.groupEnd();
      console.error(err.stack);
      stats.failed++;
      if (exitOnFail) {
        break;
      }
    }
  }
}

/** Defines options for controlling execution details of a test suite. */
export interface RunOptions {
  parallel?: boolean;
  exitOnFail?: boolean;
  only?: RegExp;
  skip?: RegExp;
}

/**
 * Runs specified test cases.
 * Parallel execution can be enabled via the boolean option; default: serial.
 */
export async function runTests({
  parallel = false,
  exitOnFail = false,
  only = /[^\s]/,
  skip = /^\s*$/
}: RunOptions = {}): Promise<void> {
  const tests: TestDefinition[] = candidates.filter(
    ({ name }): boolean => only.test(name) && !skip.test(name)
  );
  const stats: TestStats = {
    measured: 0,
    ignored: candidates.length - tests.length,
    filtered: filtered,
    passed: 0,
    failed: 0
  };
  const results: TestResults = createTestResults(tests);
  console.log(`running ${tests.length} tests`);
  if (parallel) {
    await runTestsParallel(stats, results, tests, exitOnFail);
  } else {
    await runTestsSerial(stats, tests, exitOnFail);
  }
  printResults(stats, results, parallel, exitOnFail);
  if (stats.failed) {
    // Use setTimeout to avoid the error being ignored due to unhandled
    // promise rejections being swallowed.
    setTimeout((): void => {
      console.error(`There were ${stats.failed} test failures.`);
      Deno.exit(1);
    }, 0);
  }
}

/**
 * Runs specified test cases if the enclosing script is main.
 * Execution mode is toggleable via opts.parallel, defaults to false.
 */
export async function runIfMain(
  meta: ImportMeta,
  opts?: RunOptions
): Promise<void> {
  if (meta.main) {
    return runTests(opts);
  }
}
