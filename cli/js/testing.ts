// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { red, green, bgRed, bold, white, gray, italic } from "./colors.ts";
import { exit } from "./os.ts";
import { Console } from "./console.ts";

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

export type TestFunction = () => void | Promise<void>;

export interface TestDefinition {
  fn: TestFunction;
  name: string;
}

declare global {
  // Only `var` variables show up in the `globalThis` type when doing a global
  // scope augmentation.
  // eslint-disable-next-line no-var
  var __DENO_TEST_REGISTRY: TestDefinition[];
}

let TEST_REGISTRY: TestDefinition[] = [];
if (globalThis["__DENO_TEST_REGISTRY"]) {
  TEST_REGISTRY = globalThis.__DENO_TEST_REGISTRY as TestDefinition[];
} else {
  Object.defineProperty(globalThis, "__DENO_TEST_REGISTRY", {
    enumerable: false,
    value: TEST_REGISTRY
  });
}

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
  only?: RegExp;
  skip?: RegExp;
  disableLog?: boolean;
}

export async function runTests({
  exitOnFail = false,
  only = /[^\s]/,
  skip = /^\s*$/,
  disableLog = false
}: RunTestsOptions = {}): Promise<void> {
  const testsToRun = TEST_REGISTRY.filter(
    ({ name }): boolean => only.test(name) && !skip.test(name)
  );

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
  const suiteStart = performance.now();

  for (const testCase of testCases) {
    try {
      const start = performance.now();
      await testCase.fn();
      const end = performance.now();
      testCase.timeElapsed = end - start;
      originalConsole.log(
        `${GREEN_OK}     ${testCase.name} ${promptTestTime(end - start, true)}`
      );
      stats.passed++;
    } catch (err) {
      testCase.error = err;
      originalConsole.log(`${RED_FAILED} ${testCase.name}`);
      originalConsole.log(err.stack);
      stats.failed++;
      if (exitOnFail) {
        break;
      }
    }
  }

  const suiteEnd = performance.now();

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
      `${promptTestTime(suiteEnd - suiteStart)}\n`
  );

  // TODO(bartlomieju): what's it for? Do we really need, maybe add handler for unhandled
  // promise to avoid such shenanigans
  if (stats.failed) {
    // Use setTimeout to avoid the error being ignored due to unhandled
    // promise rejections being swallowed.
    setTimeout((): void => {
      originalConsole.error(`There were ${stats.failed} test failures.`);
      testCases
        .filter(testCase => !!testCase.error)
        .forEach(testCase => {
          originalConsole.error(`${RED_BG_FAIL} ${red(testCase.name)}`);
          originalConsole.error(testCase.error);
        });
      exit(1);
    }, 0);
  }
}
