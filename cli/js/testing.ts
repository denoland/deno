// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { red, green, bgRed, gray, italic } from "./colors.ts";
import { exit } from "./ops/os.ts";
import { Console } from "./web/console.ts";

const RED_FAILED = red("FAILED");
const GREEN_OK = green("OK");
const RED_BG_FAIL = bgRed(" FAIL ");

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

type FilterFn = (testDef: TestDefinition) => boolean;

interface TestResult {
  passed: boolean;
  name: string;
  fn: TestFunction;
  skipped: boolean;
  hasRun: boolean;
  duration: number;
  error?: Error;
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

interface EndMsg {
  kind: MsgKind.End;
  stats: TestStats;
}

type RunTestsMessage = StartMsg | TestMsg | EndMsg;

// interface TestsResult {
//   results: TestResult[];
//   stats: TestStats;
// }

function testDefinitionToResult(def: TestDefinition): TestResult {
  return {
    fn: def.fn,
    name: def.name,
    passed: false,
    skipped: false,
    hasRun: false,
    duration: 0
  };
}

// TODO: implements AsyncGenerator<RunTestsMessage>
// TODO: implements PromiseLike<TestsResult>
class TestApi {
  readonly stats: TestStats;
  readonly testsToRun: TestDefinition[];
  readonly results: TestResult[];

  constructor(
    public tests: TestDefinition[],
    public filterFn: FilterFn,
    public failFast: boolean
  ) {
    this.stats = {
      filtered: 0,
      ignored: 0,
      measured: 0,
      passed: 0,
      failed: 0
    };
    this.testsToRun = tests.filter(filterFn);
    this.stats.filtered = tests.length - this.testsToRun.length;
    this.results = this.testsToRun.map(testDefinitionToResult);
  }

  async *[Symbol.asyncIterator](): AsyncIterator<RunTestsMessage> {
    yield {
      kind: MsgKind.Start,
      stats: this.stats,
      tests: this.testsToRun.length
    };

    for (const testResult of this.results) {
      let shouldBreak = false;
      try {
        const start = +new Date();
        await testResult.fn();
        testResult.duration = +new Date() - start;
        testResult.passed = true;
        this.stats.passed++;
      } catch (err) {
        testResult.passed = false;
        testResult.error = err;
        this.stats.failed++;
        if (this.failFast) {
          shouldBreak = true;
        }
      } finally {
        testResult.hasRun = true;
        yield { kind: MsgKind.Test, result: testResult };
        if (shouldBreak) {
          break;
        }
      }
    }

    yield { kind: MsgKind.End, stats: this.stats };
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

export async function runTests({
  exitOnFail = true,
  failFast = false,
  only = undefined,
  skip = undefined,
  disableLog = false
}: RunTestsOptions = {}): Promise<void> {
  const filterFn = createFilterFn(only, skip);
  const testApi = new TestApi(TEST_REGISTRY, filterFn, failFast);

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

  const suiteStart = +new Date();
  let stats: TestStats;

  for await (const testMsg of testApi) {
    if (testMsg.kind === MsgKind.Start) {
      originalConsole.log(`running ${testMsg.tests} tests`);
      continue;
    }

    if (testMsg.kind === MsgKind.Test) {
      const { result } = testMsg;

      if (result.passed) {
        originalConsole.log(
          `${GREEN_OK}     ${result.name} ${formatDuration(result.duration)}`
        );
      } else {
        originalConsole.log(`${RED_FAILED} ${result.name}`);
        originalConsole.log(result.error!.stack);
      }

      continue;
    }

    stats = testMsg.stats;
    const suiteDuration = +new Date() - suiteStart;
    // Attempting to match the output of Rust's test runner.
    originalConsole.log(
      `\ntest result: ${stats.failed ? RED_BG_FAIL : GREEN_OK} ` +
        `${stats.passed} passed; ${stats.failed} failed; ` +
        `${stats.ignored} ignored; ${stats.measured} measured; ` +
        `${stats.filtered} filtered out ` +
        `${formatDuration(suiteDuration)}\n`
    );
  }

  if (disableLog) {
    // @ts-ignore
    globalThis.console = originalConsole;
  }

  // TODO(bartlomieju): is `defer` really needed? Shouldn't unhandled
  // promise rejection be handled per test case?
  // Use defer to avoid the error being ignored due to unhandled
  // promise rejections being swallowed.
  await defer(0);

  if (stats!.failed > 0) {
    originalConsole.error(`There were ${stats!.failed} test failures.`);
    testApi.results
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
