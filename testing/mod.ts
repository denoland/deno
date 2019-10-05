// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import {
  bgRed,
  white,
  bold,
  green,
  red,
  gray,
  yellow,
  italic
} from "../fmt/colors.ts";
export type TestFunction = () => void | Promise<void>;

export interface TestDefinition {
  fn: TestFunction;
  name: string;
}

// Replacement of the global `console` function to be in silent mode
const noop = function(): void {};

// Clear the current line of the console.
// see: http://ascii-table.com/ansi-escape-sequences-vt-100.php
const CLEAR_LINE = "\x1b[2K\r";

// Save Object of the global `console` in case of silent mode
type Console = typeof window.console;
// ref https://console.spec.whatwg.org/#console-namespace
// For historical web-compatibility reasons, the namespace object for
// console must have as its [[Prototype]] an empty object, created as if
// by ObjectCreate(%ObjectPrototype%), instead of %ObjectPrototype%.
const disabledConsole = Object.create({}) as Console;
Object.assign(disabledConsole, {
  log: noop,
  debug: noop,
  info: noop,
  dir: noop,
  warn: noop,
  error: noop,
  assert: noop,
  count: noop,
  countReset: noop,
  table: noop,
  time: noop,
  timeLog: noop,
  timeEnd: noop,
  group: noop,
  groupCollapsed: noop,
  groupEnd: noop,
  clear: noop
});

const originalConsole = window.console;

function enableConsole(): void {
  window.console = originalConsole;
}

function disableConsole(): void {
  window.console = disabledConsole;
}

const encoder = new TextEncoder();
function print(txt: string, newline = true): void {
  if (newline) {
    txt += "\n";
  }
  Deno.stdout.writeSync(encoder.encode(`${txt}`));
}

declare global {
  interface Window {
    /**
     * A global property to collect all registered test cases.
     *
     * It is required because user's code can import multiple versions
     * of `testing` module.
     *
     * If test cases aren't registered in a globally shared
     * object, then imports from different versions would register test cases
     * to registry from it's respective version of `testing` module.
     */
    __DENO_TEST_REGISTRY: TestDefinition[];
  }
}

let candidates: TestDefinition[] = [];
if (window["__DENO_TEST_REGISTRY"]) {
  candidates = window.__DENO_TEST_REGISTRY as TestDefinition[];
} else {
  window["__DENO_TEST_REGISTRY"] = candidates;
}
let filterRegExp: RegExp | null;
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

export function test(t: TestDefinition): void;
export function test(fn: TestFunction): void;
export function test(name: string, fn: TestFunction): void;
export function test(
  t: string | TestDefinition | TestFunction,
  fn?: TestFunction
): void {
  let name: string;

  if (typeof t === "string") {
    if (!fn) {
      throw new Error("Missing test function");
    }
    name = t;
  } else {
    fn = typeof t === "function" ? t : t.fn;
    name = t.name;
  }

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
const GREEN_OK = green("OK");
const RED_BG_FAIL = bgRed(" FAIL ");

interface TestStats {
  filtered: number;
  ignored: number;
  measured: number;
  passed: number;
  failed: number;
}

interface TestResult {
  timeElapsed?: number;
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

function formatTestTime(time = 0): string {
  return `${time.toFixed(2)}ms`;
}

function promptTestTime(time = 0, displayWarning = false): string {
  // if time > 5s we display a warning
  // only for test time, not the full runtime
  if (displayWarning && time >= 5000) {
    return bgRed(white(bold(`(${formatTestTime(time)})`)));
  } else {
    return gray(italic(`(${formatTestTime(time)})`));
  }
}

function report(result: TestResult): void {
  if (result.ok) {
    print(
      `${GREEN_OK}     ${result.name} ${promptTestTime(
        result.timeElapsed,
        true
      )}`
    );
  } else if (result.error) {
    print(`${RED_FAILED} ${result.name}\n${result.error.stack}`);
  } else {
    print(`test ${result.name} ... unresolved`);
  }
  result.printed = true;
}

function printFailedSummary(results: TestResults): void {
  results.cases.forEach((v): void => {
    if (!v.ok) {
      console.error(`${RED_BG_FAIL} ${red(v.name)}`);
      console.error(v.error);
    }
  });
}

function printResults(
  stats: TestStats,
  results: TestResults,
  flush: boolean,
  exitOnFail: boolean,
  timeElapsed: number
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
  print(
    `\ntest result: ${stats.failed ? RED_BG_FAIL : GREEN_OK} ` +
      `${stats.passed} passed; ${stats.failed} failed; ` +
      `${stats.ignored} ignored; ${stats.measured} measured; ` +
      `${stats.filtered} filtered out ` +
      `${promptTestTime(timeElapsed)}\n`
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
    const start = performance.now();
    await fn();
    const end = performance.now();
    stats.passed++;
    result.ok = true;
    result.timeElapsed = end - start;
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
  results: TestResults,
  tests: TestDefinition[],
  exitOnFail: boolean,
  disableLog: boolean
): Promise<void> {
  for (const { fn, name } of tests) {
    // Displaying the currently running test if silent mode
    if (disableLog) {
      print(`${yellow("RUNNING")} ${name}`, false);
    }
    try {
      const start = performance.now();
      await fn();
      const end = performance.now();
      if (disableLog) {
        // Rewriting the current prompt line to erase `running ....`
        print(CLEAR_LINE, false);
      }
      stats.passed++;
      print(
        GREEN_OK + "     " + name + " " + promptTestTime(end - start, true)
      );
      results.cases.forEach((v): void => {
        if (v.name === name) {
          v.ok = true;
          v.printed = true;
        }
      });
    } catch (err) {
      if (disableLog) {
        print(CLEAR_LINE, false);
      }
      print(`${RED_FAILED} ${name}`);
      print(err.stack);
      stats.failed++;
      results.cases.forEach((v): void => {
        if (v.name === name) {
          v.error = err;
          v.ok = false;
          v.printed = true;
        }
      });
      if (exitOnFail) {
        break;
      }
    }
  }
}

/** Defines options for controlling execution details of a test suite. */
export interface RunTestsOptions {
  parallel?: boolean;
  exitOnFail?: boolean;
  only?: RegExp;
  skip?: RegExp;
  disableLog?: boolean;
}

/**
 * Runs specified test cases.
 * Parallel execution can be enabled via the boolean option; default: serial.
 */
// TODO: change return type to `Promise<boolean>` - ie. don't
// exit but return value
export async function runTests({
  parallel = false,
  exitOnFail = false,
  only = /[^\s]/,
  skip = /^\s*$/,
  disableLog = false
}: RunTestsOptions = {}): Promise<void> {
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
  print(`running ${tests.length} tests`);
  const start = performance.now();
  if (Deno.args.includes("--quiet")) {
    disableLog = true;
  }
  if (disableLog) {
    disableConsole();
  }
  if (parallel) {
    await runTestsParallel(stats, results, tests, exitOnFail);
  } else {
    await runTestsSerial(stats, results, tests, exitOnFail, disableLog);
  }
  const end = performance.now();
  if (disableLog) {
    enableConsole();
  }
  printResults(stats, results, parallel, exitOnFail, end - start);
  if (stats.failed) {
    // Use setTimeout to avoid the error being ignored due to unhandled
    // promise rejections being swallowed.
    setTimeout((): void => {
      console.error(`There were ${stats.failed} test failures.`);
      printFailedSummary(results);
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
  opts?: RunTestsOptions
): Promise<void> {
  if (meta.main) {
    return runTests(opts);
  }
}
