// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { red, green, bgRed, gray, italic } from "./colors.ts";
import { exit } from "./ops/os.ts";
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
  raw?: boolean;
}

interface RunTestsMessage {
  kind: "start" | "testResult" | "end";
  start?: {
    tests: number;
  };
  stats?: TestStats;
  result?: TestCase;
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

class RunTestsIterable implements AsyncIterableIterator<RunTestsMessage> {
  readonly testsToRun: TestDefinition[];
  readonly testCases: TestCase[];
  failFast: boolean;
  currentTest = -1;
  done = false;
  stats: TestStats = {
    measured: 0,
    ignored: 0,
    filtered: 0,
    passed: 0,
    failed: 0
  };

  constructor(
    tests: TestDefinition[],
    only: undefined | string | RegExp,
    skip: undefined | string | RegExp,
    failFast: boolean
  ) {
    this.testsToRun = filterTests(tests, only, skip);
    this.failFast = failFast;
    this.testCases = this.testsToRun.map(
      ({ name, fn }): TestCase => {
        return {
          name,
          fn,
          timeElapsed: 0,
          error: undefined
        };
      }
    );
  }

  async next(): Promise<IteratorResult<RunTestsMessage>> {
    if (this.currentTest === -1) {
      this.currentTest = 0;
      return {
        done: false,
        value: {
          kind: "start",
          start: {
            tests: this.testsToRun.length
          }
        }
      };
    }

    if (this.done) {
      return {
        done: true,
        value: {
          kind: "end",
          stats: this.stats
        }
      };
    }

    const testCase = this.testCases[this.currentTest];
    if (testCase === undefined) {
      throw new Error(`Missing test case: ${this.currentTest}`);
    }

    // TODO: using permormance here doesn't make sense
    try {
      const start = +new Date();
      await testCase.fn();
      testCase.timeElapsed = +new Date() - start;
      this.stats.passed++;
    } catch (err) {
      testCase.error = err;
      this.stats.failed++;
      if (this.failFast) {
        this.done = true;
      }
    }

    this.currentTest++;
    if (this.currentTest === this.testCases.length) {
      this.done = true;
    }

    return {
      done: false,
      value: {
        kind: "testResult",
        result: testCase
      }
    };
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<RunTestsMessage> {
    return this;
  }
}

export async function runTests({
  exitOnFail = true,
  failFast = false,
  only = undefined,
  skip = undefined,
  disableLog = false,
  raw = false
}: RunTestsOptions = {}): Promise<void> {
  const iterator = new RunTestsIterable(TEST_REGISTRY, only, skip, exitOnFail);

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

  if (raw) {
    for await (const msg of iterator) {
      Deno.core.print(`${JSON.stringify(msg)}\n`);
    }
    const msg = await iterator.next();
    Deno.core.print(`${JSON.stringify(msg.value)}\n`);
    return;
  }

  let stats: TestStats;
  const { value } = await iterator.next();
  if (value.kind !== "start") {
    throw Error("Bad message");
  }
  stats = value.stats;

  const RED_FAILED = red("FAILED");
  const GREEN_OK = green("OK");
  const RED_BG_FAIL = bgRed(" FAIL ");

  originalConsole.log(`running ${testsToRun.length} tests`);
  const suiteStart = +new Date();

  for (const msg of iterator) {
    if (msg.kind === "end") {
      stats = msg.stats as TestStats;
      break;
    }

    const testResult = msg.result as TestCase;
    if (testResult.error) {
      originalConsole.log(`${RED_FAILED} ${testResult.name}`);
      originalConsole.log(testResult.error.stack);
    } else {
      originalConsole.log(
        `${GREEN_OK}     ${testResult.name} ${promptTestTime(
          testResult.timeElapsed,
          true
        )}`
      );
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
    iterator.testCases
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
