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

interface TestResult {
  name: string;
  fn: TestFunction;
  hasRun: boolean;
  duration: number;
  failed: boolean;
  error?: Error;
}

export interface RunTestsOptions {
  exitOnFail?: boolean;
  failFast?: boolean;
  only?: string | RegExp;
  skip?: string | RegExp;
  disableLog?: boolean;
  json?: boolean;
}

enum MsgKind {
  Start = "start",
  Test = "test",
  End = "end"
}

interface StartMsg {
  kind: MsgKind.Start;
  tests: number;
  stats: TestStats;
}

interface TestMsg {
  kind: MsgKind.Test;
  result: TestResult;
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

interface EndMsg {
  kind: MsgKind.End;
  stats: TestStats;
}

type RunTestsMessage = StartMsg | TestMsg | EndMsg;

class RunTestsIterable implements AsyncIterableIterator<RunTestsMessage> {
  readonly testsToRun: TestDefinition[];
  readonly testResults: TestResult[];

  private hasStarted = false;
  private hasFinished = false;
  private currentTestIndex = 0;

  failFast: boolean;

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
    this.testResults = this.testsToRun.map(
      ({ name, fn }): TestResult => {
        return {
          name,
          fn,
          duration: 0,
          hasRun: false,
          failed: false,
          error: undefined
        };
      }
    );
  }

  async next(): Promise<IteratorResult<RunTestsMessage>> {
    if (!this.hasStarted) {
      this.hasStarted = true;
      return {
        done: false,
        value: {
          kind: MsgKind.Start,
          tests: this.testsToRun.length,
          stats: this.stats
        }
      };
    }

    if (this.hasFinished) {
      return {
        done: true,
        value: {
          kind: MsgKind.End,
          stats: this.stats
        }
      };
    }

    const testResult = this.testResults[this.currentTestIndex];
    if (testResult === undefined) {
      throw new Error(`Missing test case: ${this.currentTestIndex}`);
    }

    if (testResult.hasRun) {
      throw new Error(`Trying to run test case again: ${testResult.name}`);
    }

    // TODO(bartlomieju): using permformance here doesn't make sense, as there
    // is no guarantee that we're running with `--allow-hr` perm
    try {
      const start = +new Date();
      await testResult.fn();
      testResult.duration = +new Date() - start;
      this.stats.passed++;
    } catch (err) {
      testResult.failed = true;
      testResult.error = err;
      this.stats.failed++;
      if (this.failFast) {
        this.hasFinished = true;
      }
    } finally {
      testResult.hasRun = true;
    }

    this.currentTestIndex++;
    if (this.currentTestIndex === this.testResults.length) {
      this.hasFinished = true;
    }

    return {
      done: false,
      value: {
        kind: MsgKind.Test,
        result: testResult
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
  json = false
}: RunTestsOptions = {}): Promise<void> {
  const iterator = new RunTestsIterable(TEST_REGISTRY, only, skip, failFast);

  // @ts-ignore
  const originalConsole = globalThis.console;
  // TODO(bartlomieju): add option to capture output of test
  // cases and display it if test fails (like --nopcature in Rust)
  const disabledConsole = new Console(
    (_x: string, _isErr?: boolean): void => {}
  );

  if (disableLog || json) {
    // @ts-ignore
    globalThis.console = disabledConsole;
  }

  if (json) {
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

  originalConsole.log(`running ${iterator.testResults.length} tests`);
  const suiteStart = +new Date();

  for await (const msg of iterator) {
    if (msg.kind === "end") {
      stats = msg.stats as TestStats;
      break;
    }

    const testResult = msg.result as TestResult;
    if (testResult.failed) {
      originalConsole.log(`${RED_FAILED} ${testResult.name}`);
      originalConsole.log(testResult.error!.stack);
    } else {
      originalConsole.log(
        `${GREEN_OK}     ${testResult.name} ${formatDuration(
          testResult.duration
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
    iterator.testResults
      .filter(result => result.failed)
      .forEach(result => {
        originalConsole.error(`${RED_BG_FAIL} ${red(result.name)}`);
        originalConsole.error(result.error);
      });

    if (exitOnFail) {
      exit(1);
    }
  }
}
