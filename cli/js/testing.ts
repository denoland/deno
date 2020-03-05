// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { red, green, bgRed, gray, italic } from "./colors.ts";
import { exit } from "./os.ts";
import { Console } from "./console.ts";

function formatDuration(time = 0): string {
  const timeStr = `(${time}ms)`;
  return gray(italic(timeStr));
}

function defer(n: number): Promise<void> {
  return new Promise((resolve: () => void, _) => {
    setTimeout(resolve, n);
  });
}

export type TestFunction = () => void | Promise<void>;

export interface TestDefinition {
  fn: TestFunction;
  name: string;
}

const TEST_REGISTRY: TestDefinition[] = [];

export function test(t: TestDefinition): void;
export function test(fn: TestFunction): void;
export function test(name: string, fn: TestFunction): void;
// Main test function provided by Deno, as you can see it merely
// creates a new object with "name" and "fn" fields.
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
    if (!name) {
      throw new Error("The name of test case can't be empty");
    }
  } else if (typeof t === "function") {
    fn = t;
    name = t.name;
    if (!name) {
      throw new Error("Test function can't be anonymous");
    }
  } else {
    fn = t.fn;
    if (!fn) {
      throw new Error("Missing test function");
    }
    name = t.name;
    if (!name) {
      throw new Error("The name of test case can't be empty");
    }
  }

  TEST_REGISTRY.push({ fn, name });
}

interface TestStats {
  filtered: number;
  ignored: number;
  measured: number;
  passed: number;
  failed: number;
}

interface TestCase {
  name: string;
  fn: TestFunction;
  timeElapsed?: number;
  error?: Error;
}

export interface RunTestsOptions {
  exitOnFail?: boolean;
  failFast?: boolean;
  only?: string | RegExp;
  skip?: string | RegExp;
  disableLog?: boolean;
}

function filterTests(
  tests: TestDefinition[],
  only: undefined | string | RegExp,
  skip: undefined | string | RegExp
): TestDefinition[] {
  return tests.filter((def: TestDefinition): boolean => {
    let passes = true;

    if (only) {
      if (only instanceof RegExp) {
        passes = passes && only.test(def.name);
      } else {
        passes = passes && def.name.includes(only);
      }
    }

    if (skip) {
      if (skip instanceof RegExp) {
        passes = passes && !skip.test(def.name);
      } else {
        passes = passes && !def.name.includes(skip);
      }
    }

    return passes;
  });
}

export async function runTests({
  exitOnFail = true,
  failFast = false,
  only = undefined,
  skip = undefined,
  disableLog = false
}: RunTestsOptions = {}): Promise<void> {
  const testsToRun = filterTests(TEST_REGISTRY, only, skip);

  const stats: TestStats = {
    measured: 0,
    ignored: 0,
    filtered: 0,
    passed: 0,
    failed: 0
  };

  const testCases = testsToRun.map(
    ({ name, fn }): TestCase => {
      return {
        name,
        fn,
        timeElapsed: 0,
        error: undefined
      };
    }
  );

  // @ts-ignore
  const originalConsole = globalThis.console;
  // TODO(bartlomieju): add option to capture output of test
  // cases and display it if test fails (like --nopcature in Rust)
  const disabledConsole = new Console(
    (_x: string, _isErr?: boolean): void => {}
  );

  if (disableLog) {
    // @ts-ignore
    globalThis.console = disabledConsole;
  }

  const RED_FAILED = red("FAILED");
  const GREEN_OK = green("OK");
  const RED_BG_FAIL = bgRed(" FAIL ");

  originalConsole.log(`running ${testsToRun.length} tests`);
  const suiteStart = +new Date();

  for (const testCase of testCases) {
    try {
      const start = +new Date();
      await testCase.fn();
      testCase.timeElapsed = +new Date() - start;
      originalConsole.log(
        `${GREEN_OK}     ${testCase.name} ${formatDuration(
          testCase.timeElapsed
        )}`
      );
      stats.passed++;
    } catch (err) {
      testCase.error = err;
      originalConsole.log(`${RED_FAILED} ${testCase.name}`);
      originalConsole.log(err.stack);
      stats.failed++;
      if (failFast) {
        break;
      }
    }
  }

  const suiteDuration = +new Date() - suiteStart;

  if (disableLog) {
    // @ts-ignore
    globalThis.console = originalConsole;
  }

  // Attempting to match the output of Rust's test runner.
  originalConsole.log(
    `\ntest result: ${stats.failed ? RED_BG_FAIL : GREEN_OK} ` +
      `${stats.passed} passed; ${stats.failed} failed; ` +
      `${stats.ignored} ignored; ${stats.measured} measured; ` +
      `${stats.filtered} filtered out ` +
      `${formatDuration(suiteDuration)}\n`
  );

  // TODO(bartlomieju): is `defer` really needed? Shouldn't unhandled
  // promise rejection be handled per test case?
  // Use defer to avoid the error being ignored due to unhandled
  // promise rejections being swallowed.
  await defer(0);

  if (stats.failed > 0) {
    originalConsole.error(`There were ${stats.failed} test failures.`);
    testCases
      .filter(testCase => !!testCase.error)
      .forEach(testCase => {
        originalConsole.error(`${RED_BG_FAIL} ${red(testCase.name)}`);
        originalConsole.error(testCase.error);
      });

    if (exitOnFail) {
      exit(1);
    }
  }
}
