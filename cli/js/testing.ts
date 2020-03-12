// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { red, green, bgRed, gray, italic } from "./colors.ts";
import { exit } from "./ops/os.ts";
import { Console } from "./web/console.ts";

const RED_FAILED = red("FAILED");
const GREEN_OK = green("OK");
const RED_BG_FAIL = bgRed(" FAIL ");
const disabledConsole = new Console((_x: string, _isErr?: boolean): void => {});

function formatDuration(time = 0): string {
  const timeStr = `(${time}ms)`;
  return gray(italic(timeStr));
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
  duration: number;
  results: TestResult[];
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

    const suiteStart = +new Date();
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
    const duration = +new Date() - suiteStart;

    yield {
      kind: MsgKind.End,
      results: this.results,
      stats: this.stats,
      duration
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
  start(msg: StartMsg): Promise<void>;
  test(msg: TestMsg): Promise<void>;
  end(msg: EndMsg): Promise<void>;
}

class ConsoleReporter implements TestReporter {
  constructor(private console: Console) {}

  async start(msg: StartMsg): Promise<void> {
    this.console.log(`running ${msg.tests} tests`);
  }

  async test(msg: TestMsg): Promise<void> {
    const { result } = msg;

    if (result.passed) {
      this.console.log(
        `${GREEN_OK}     ${result.name} ${formatDuration(result.duration)}`
      );
    } else {
      this.console.log(`${RED_FAILED} ${result.name}`);
      this.console.log(result.error!.stack);
    }
  }

  async end(msg: EndMsg): Promise<void> {
    const stats = msg.stats;
    // Attempting to match the output of Rust's test runner.
    this.console.log(
      `\ntest result: ${stats.failed ? RED_BG_FAIL : GREEN_OK} ` +
        `${stats.passed} passed; ${stats.failed} failed; ` +
        `${stats.ignored} ignored; ${stats.measured} measured; ` +
        `${stats.filtered} filtered out ` +
        `${formatDuration(msg.duration)}\n`
    );

    if (stats!.failed > 0) {
      this.console.log(`There were ${stats!.failed} test failures.`);
      msg.results
        .filter(testCase => !!testCase.error)
        .forEach(testCase => {
          this.console.log(`${RED_BG_FAIL} ${red(testCase.name)}`);
          this.console.log(testCase.error);
        });
    }
  }
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

  if (disableLog) {
    // @ts-ignore
    globalThis.console = disabledConsole;
  }

  const reporter = new ConsoleReporter(originalConsole);

  let stats: TestStats;

  for await (const testMsg of testApi) {
    switch (testMsg.kind) {
      case MsgKind.Start:
        await reporter.start(testMsg);
        continue;
      case MsgKind.Test:
        await reporter.test(testMsg);
        continue;
      case MsgKind.End:
        stats = testMsg.stats;
        await reporter.end(testMsg);
        continue;
    }
  }

  if (disableLog) {
    // @ts-ignore
    globalThis.console = originalConsole;
  }

  if (stats!.failed > 0 && exitOnFail) {
    exit(1);
  }
}
