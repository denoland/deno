// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { gray, green, italic, red, yellow } from "./colors.ts";
import { exit } from "./ops/os.ts";
import { Console, stringifyArgs } from "./web/console.ts";
import { stdout } from "./files.ts";
import { TextEncoder } from "./web/text_encoding.ts";
import { metrics } from "./ops/runtime.ts";
import { resources } from "./ops/resources.ts";
import { assert } from "./util.ts";

const RED_FAILED = red("FAILED");
const GREEN_OK = green("ok");
const YELLOW_IGNORED = yellow("ignored");
const disabledConsole = new Console((): void => {});

function formatDuration(time = 0): string {
  const timeStr = `(${time}ms)`;
  return gray(italic(timeStr));
}

// Wrap `TestFunction` in additional assertion that makes sure
// the test case does not leak async "ops" - ie. number of async
// completed ops after the test is the same as number of dispatched
// ops. Note that "unref" ops are ignored since in nature that are
// optional.
function assertOps(fn: TestFunction): TestFunction {
  return async function asyncOpSanitizer(): Promise<void> {
    const pre = metrics();
    await fn();
    const post = metrics();
    // We're checking diff because one might spawn HTTP server in the background
    // that will be a pending async op before test starts.
    const dispatchedDiff = post.opsDispatchedAsync - pre.opsDispatchedAsync;
    const completedDiff = post.opsCompletedAsync - pre.opsCompletedAsync;
    assert(
      dispatchedDiff === completedDiff,
      `Test case is leaking async ops.
Before:
  - dispatched: ${pre.opsDispatchedAsync}
  - completed: ${pre.opsCompletedAsync}
After: 
  - dispatched: ${post.opsDispatchedAsync}
  - completed: ${post.opsCompletedAsync}`
    );
  };
}

// Wrap `TestFunction` in additional assertion that makes sure
// the test case does not "leak" resources - ie. resource table after
// the test has exactly the same contents as before the test.
function assertResources(fn: TestFunction): TestFunction {
  return async function resourceSanitizer(): Promise<void> {
    const pre = resources();
    await fn();
    const post = resources();

    const preStr = JSON.stringify(pre, null, 2);
    const postStr = JSON.stringify(post, null, 2);
    const msg = `Test case is leaking resources.
Before: ${preStr}
After: ${postStr}`;
    assert(preStr === postStr, msg);
  };
}

export type TestFunction = () => void | Promise<void>;

export interface TestDefinition {
  fn: TestFunction;
  name: string;
  ignore?: boolean;
  disableOpSanitizer?: boolean;
  disableResourceSanitizer?: boolean;
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
  let testDef: TestDefinition;

  if (typeof t === "string") {
    if (!fn || typeof fn != "function") {
      throw new TypeError("Missing test function");
    }
    if (!t) {
      throw new TypeError("The test name can't be empty");
    }
    testDef = { fn: fn as TestFunction, name: t, ignore: false };
  } else if (typeof t === "function") {
    if (!t.name) {
      throw new TypeError("The test function can't be anonymous");
    }
    testDef = { fn: t, name: t.name, ignore: false };
  } else {
    if (!t.fn) {
      throw new TypeError("Missing test function");
    }
    if (!t.name) {
      throw new TypeError("The test name can't be empty");
    }
    testDef = { ...t, ignore: Boolean(t.ignore) };
  }

  if (testDef.disableOpSanitizer !== true) {
    testDef.fn = assertOps(testDef.fn);
  }

  if (testDef.disableResourceSanitizer !== true) {
    testDef.fn = assertResources(testDef.fn);
  }

  TEST_REGISTRY.push(testDef);
}

interface TestStats {
  filtered: number;
  ignored: number;
  measured: number;
  passed: number;
  failed: number;
}

export interface RunTestsOptions {
  exitOnFail?: boolean;
  failFast?: boolean;
  only?: string | RegExp;
  skip?: string | RegExp;
  disableLog?: boolean;
  reporter?: TestReporter;
}

enum TestStatus {
  Passed = "passed",
  Failed = "failed",
  Ignored = "ignored",
}

interface TestResult {
  name: string;
  status: TestStatus;
  duration: number;
  error?: Error;
}

export enum TestEvent {
  Start = "start",
  TestStart = "testStart",
  TestEnd = "testEnd",
  End = "end",
}

interface TestEventStart {
  kind: TestEvent.Start;
  tests: number;
}

interface TestEventTestStart {
  kind: TestEvent.TestStart;
  name: string;
}

interface TestEventTestEnd {
  kind: TestEvent.TestEnd;
  result: TestResult;
}

interface TestEventEnd {
  kind: TestEvent.End;
  stats: TestStats;
  duration: number;
  results: TestResult[];
}

// TODO: already implements AsyncGenerator<RunTestsMessage>, but add as "implements to class"
// TODO: implements PromiseLike<TestsResult>
class TestApi {
  readonly testsToRun: TestDefinition[];
  readonly stats: TestStats = {
    filtered: 0,
    ignored: 0,
    measured: 0,
    passed: 0,
    failed: 0,
  };

  constructor(
    public tests: TestDefinition[],
    public filterFn: (def: TestDefinition) => boolean,
    public failFast: boolean
  ) {
    this.testsToRun = tests.filter(filterFn);
    this.stats.filtered = tests.length - this.testsToRun.length;
  }

  async *[Symbol.asyncIterator](): AsyncIterator<
    TestEventStart | TestEventTestStart | TestEventTestEnd | TestEventEnd
  > {
    yield {
      kind: TestEvent.Start,
      tests: this.testsToRun.length,
    };

    const results: TestResult[] = [];
    const suiteStart = +new Date();
    for (const { name, fn, ignore } of this.testsToRun) {
      const result: Partial<TestResult> = { name, duration: 0 };
      yield { kind: TestEvent.TestStart, name };
      if (ignore) {
        result.status = TestStatus.Ignored;
        this.stats.ignored++;
      } else {
        const start = +new Date();
        try {
          await fn();
          result.status = TestStatus.Passed;
          this.stats.passed++;
        } catch (err) {
          result.status = TestStatus.Failed;
          result.error = err;
          this.stats.failed++;
        } finally {
          result.duration = +new Date() - start;
        }
      }
      yield { kind: TestEvent.TestEnd, result: result as TestResult };
      results.push(result as TestResult);
      if (this.failFast && result.error != null) {
        break;
      }
    }

    const duration = +new Date() - suiteStart;

    yield {
      kind: TestEvent.End,
      stats: this.stats,
      results,
      duration,
    };
  }
}

function createFilterFn(
  only: undefined | string | RegExp,
  skip: undefined | string | RegExp
): (def: TestDefinition) => boolean {
  return (def: TestDefinition): boolean => {
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
  };
}

interface TestReporter {
  start(msg: TestEventStart): Promise<void>;
  testStart(msg: TestEventTestStart): Promise<void>;
  testEnd(msg: TestEventTestEnd): Promise<void>;
  end(msg: TestEventEnd): Promise<void>;
}

export class ConsoleTestReporter implements TestReporter {
  start(event: TestEventStart): Promise<void> {
    ConsoleTestReporter.log(`running ${event.tests} tests`);
    return Promise.resolve();
  }

  testStart(event: TestEventTestStart): Promise<void> {
    const { name } = event;

    ConsoleTestReporter.log(`test ${name} ... `, true);
    return Promise.resolve();
  }

  testEnd(event: TestEventTestEnd): Promise<void> {
    const { result } = event;

    switch (result.status) {
      case TestStatus.Passed:
        ConsoleTestReporter.log(
          `${GREEN_OK} ${formatDuration(result.duration)}`
        );
        break;
      case TestStatus.Failed:
        ConsoleTestReporter.log(
          `${RED_FAILED} ${formatDuration(result.duration)}`
        );
        break;
      case TestStatus.Ignored:
        ConsoleTestReporter.log(
          `${YELLOW_IGNORED} ${formatDuration(result.duration)}`
        );
        break;
    }

    return Promise.resolve();
  }

  end(event: TestEventEnd): Promise<void> {
    const { stats, duration, results } = event;
    // Attempting to match the output of Rust's test runner.
    const failedTests = results.filter((r) => r.error);

    if (failedTests.length > 0) {
      ConsoleTestReporter.log(`\nfailures:\n`);

      for (const result of failedTests) {
        ConsoleTestReporter.log(`${result.name}`);
        ConsoleTestReporter.log(`${stringifyArgs([result.error!])}`);
        ConsoleTestReporter.log("");
      }

      ConsoleTestReporter.log(`failures:\n`);

      for (const result of failedTests) {
        ConsoleTestReporter.log(`\t${result.name}`);
      }
    }

    ConsoleTestReporter.log(
      `\ntest result: ${stats.failed ? RED_FAILED : GREEN_OK}. ` +
        `${stats.passed} passed; ${stats.failed} failed; ` +
        `${stats.ignored} ignored; ${stats.measured} measured; ` +
        `${stats.filtered} filtered out ` +
        `${formatDuration(duration)}\n`
    );

    return Promise.resolve();
  }

  static encoder = new TextEncoder();

  static log(msg: string, noNewLine = false): Promise<void> {
    if (!noNewLine) {
      msg += "\n";
    }

    // Using `stdout` here because it doesn't force new lines
    // compared to `console.log`; `core.print` on the other hand
    // is line-buffered and doesn't output message without newline
    stdout.writeSync(ConsoleTestReporter.encoder.encode(msg));
    return Promise.resolve();
  }
}

export async function runTests({
  exitOnFail = true,
  failFast = false,
  only = undefined,
  skip = undefined,
  disableLog = false,
  reporter = undefined,
}: RunTestsOptions = {}): Promise<{
  results: TestResult[];
  stats: TestStats;
  duration: number;
}> {
  const filterFn = createFilterFn(only, skip);
  const testApi = new TestApi(TEST_REGISTRY, filterFn, failFast);

  if (!reporter) {
    reporter = new ConsoleTestReporter();
  }

  // @ts-ignore
  const originalConsole = globalThis.console;

  if (disableLog) {
    // @ts-ignore
    globalThis.console = disabledConsole;
  }

  let endMsg: TestEventEnd;

  for await (const testMsg of testApi) {
    switch (testMsg.kind) {
      case TestEvent.Start:
        await reporter.start(testMsg);
        continue;
      case TestEvent.TestStart:
        await reporter.testStart(testMsg);
        continue;
      case TestEvent.TestEnd:
        await reporter.testEnd(testMsg);
        continue;
      case TestEvent.End:
        endMsg = testMsg;
        delete endMsg!.kind;
        await reporter.end(testMsg);
        continue;
    }
  }

  if (disableLog) {
    // @ts-ignore
    globalThis.console = originalConsole;
  }

  if (endMsg!.stats.failed > 0 && exitOnFail) {
    exit(1);
  }

  return endMsg!;
}
