// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { green, red } from "../colors/mod.ts";
import { assertEqual as prettyAssertEqual } from "./pretty.ts";
import {
  assert as assertImport,
  equal as AssertEqual,
  assertStrictEq,
  assertStrContains,
  assertMatch,
  fail,
  assertThrows,
  assertThrowsAsync
} from "./asserts.ts";

export function equal(c: unknown, d: unknown): boolean {
  const seen = new Map();
  return (function compare(a: unknown, b: unknown) {
    if (Object.is(a, b)) {
      return true;
    }
    if (a && typeof a === "object" && b && typeof b === "object") {
      if (seen.get(a) === b) {
        return true;
      }
      if (Object.keys(a || {}).length !== Object.keys(b || {}).length) {
        return false;
      }
      const merged = { ...a, ...b };
      for (const key in merged) {
        type Key = keyof typeof merged;
        if (!compare(a && a[key as Key], b && b[key as Key])) {
          return false;
        }
      }
      seen.set(a, b);
      return true;
    }
    return false;
  })(c, d);
}

const assertions = {
  assert: assertImport,
  equal: AssertEqual,
  strictEqual: assertStrictEq,
  assertStrContains: assertStrContains,
  assertMatch: assertMatch,
  fail: fail,
  throws: assertThrows,
  throwsAsync: assertThrowsAsync
};

type Assert = typeof assertions.assert & typeof assertions;

// Decorate assertions.assert with all the assertions
Object.assign(assertions.assert, assertions);

export const assert = assertions.assert as Assert;

/**
 * Alias to pretty.assertEqual
 * @deprecated
 */
export const assertEqual = prettyAssertEqual;

export type TestFunction = () => void | Promise<void>;

export interface TestDefinition {
  fn: TestFunction;
  name: string;
}

export const exitOnFail = true;

let filterRegExp: RegExp | null;
const tests: TestDefinition[] = [];

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
    tests.push({ fn, name });
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
  error: Error;
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
      acc.cases.set(i, { name, printed: false, ok: false, error: null });
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
  flush: boolean
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
  const curIndex: number = results.keys.get(name);
  if (curIndex === 0) {
    return true;
  }
  return results.cases.get(curIndex - 1).printed;
}

async function createTestCase(
  stats: TestStats,
  results: TestResults,
  { fn, name }: TestDefinition
): Promise<void> {
  const result: TestResult = results.cases.get(results.keys.get(name));
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
  tests: TestDefinition[]
): Array<Promise<void>> {
  return tests.map(createTestCase.bind(null, stats, results));
}

async function runTestsParallel(
  stats: TestStats,
  results: TestResults,
  tests: TestDefinition[]
): Promise<void> {
  try {
    await Promise.all(initTestCases(stats, results, tests));
  } catch (_) {
    // The error was thrown to stop awaiting all promises if exitOnFail === true
    // stats.failed has been incremented and the error stored in results
  }
}

async function runTestsSerial(
  stats: TestStats,
  tests: TestDefinition[]
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
}

/**
 * Runs specified test cases.
 * Parallel execution can be enabled via the boolean option; default: serial.
 */
export async function runTests({ parallel = false }: RunOptions = {}): Promise<
  void
> {
  const stats: TestStats = {
    measured: 0,
    ignored: 0,
    filtered: filtered,
    passed: 0,
    failed: 0
  };
  const results: TestResults = createTestResults(tests);
  console.log(`running ${tests.length} tests`);
  if (parallel) {
    await runTestsParallel(stats, results, tests);
  } else {
    await runTestsSerial(stats, tests);
  }
  printResults(stats, results, parallel);
  if (stats.failed) {
    // Use setTimeout to avoid the error being ignored due to unhandled
    // promise rejections being swallowed.
    setTimeout(() => {
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
