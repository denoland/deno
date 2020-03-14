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

export interface RunTestsOptions {
  exitOnFail?: boolean;
  failFast?: boolean;
  only?: string | RegExp;
  skip?: string | RegExp;
  disableLog?: boolean;
  reporter?: TestReporter;
}

interface TestResult {
  passed: boolean;
  name: string;
  skipped: boolean;
  hasRun: boolean;
  duration: number;
  error?: Error;
}

interface TestCase {
  result: TestResult;
  fn: TestFunction;
}

export enum TestEvent {
  Start = "start",
  Result = "result",
  End = "end"
}

interface TestEventStart {
  kind: TestEvent.Start;
  tests: number;
}

interface TestEventResult {
  kind: TestEvent.Result;
  result: TestResult;
}

interface TestEventEnd {
  kind: TestEvent.End;
  stats: TestStats;
  duration: number;
  results: TestResult[];
}

function testDefinitionToTestCase(def: TestDefinition): TestCase {
  return {
    fn: def.fn,
    result: {
      name: def.name,
      passed: false,
      skipped: false,
      hasRun: false,
      duration: 0
    }
  };
}

// TODO: already implements AsyncGenerator<RunTestsMessage>, but add as "implements to class"
// TODO: implements PromiseLike<TestsResult>
class TestApi {
  readonly testsToRun: TestDefinition[];
  readonly testCases: TestCase[];
  readonly stats: TestStats = {
    filtered: 0,
    ignored: 0,
    measured: 0,
    passed: 0,
    failed: 0
  };

  constructor(
    public tests: TestDefinition[],
    public filterFn: (def: TestDefinition) => boolean,
    public failFast: boolean
  ) {
    this.testsToRun = tests.filter(filterFn);
    this.stats.filtered = tests.length - this.testsToRun.length;
    this.testCases = this.testsToRun.map(testDefinitionToTestCase);
  }

  async *[Symbol.asyncIterator](): AsyncIterator<
    TestEventStart | TestEventResult | TestEventEnd
  > {
    yield {
      kind: TestEvent.Start,
      tests: this.testsToRun.length
    };

    const suiteStart = +new Date();
    for (const testCase of this.testCases) {
      const { fn, result } = testCase;
      let shouldBreak = false;
      try {
        const start = +new Date();
        await fn();
        result.duration = +new Date() - start;
        result.passed = true;
        this.stats.passed++;
      } catch (err) {
        result.passed = false;
        result.error = err;
        this.stats.failed++;
        shouldBreak = this.failFast;
      } finally {
        result.hasRun = true;
        yield { kind: TestEvent.Result, result };
        if (shouldBreak) {
          break;
        }
      }
    }

    const duration = +new Date() - suiteStart;
    const results = this.testCases.map(r => r.result);

    yield {
      kind: TestEvent.End,
      stats: this.stats,
      results,
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
  start(msg: TestEventStart): Promise<void>;
  result(msg: TestEventResult): Promise<void>;
  end(msg: TestEventEnd): Promise<void>;
}

export class ConsoleTestReporter implements TestReporter {
  private console: Console;
  constructor() {
    this.console = globalThis.console as Console;
  }

  async start(event: TestEventStart): Promise<void> {
    this.console.log(`running ${event.tests} tests`);
  }

  async result(event: TestEventResult): Promise<void> {
    const { result } = event;

    if (result.passed) {
      this.console.log(
        `${GREEN_OK}     ${result.name} ${formatDuration(result.duration)}`
      );
    } else {
      this.console.log(`${RED_FAILED} ${result.name}`);
      this.console.log(result.error!);
    }
  }

  async end(event: TestEventEnd): Promise<void> {
    const { stats, duration } = event;
    // Attempting to match the output of Rust's test runner.
    this.console.log(
      `\ntest result: ${stats.failed ? RED_BG_FAIL : GREEN_OK} ` +
        `${stats.passed} passed; ${stats.failed} failed; ` +
        `${stats.ignored} ignored; ${stats.measured} measured; ` +
        `${stats.filtered} filtered out ` +
        `${formatDuration(duration)}\n`
    );
  }
}

export async function runTests({
  exitOnFail = true,
  failFast = false,
  only = undefined,
  skip = undefined,
  disableLog = false,
  reporter = undefined
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
      case TestEvent.Result:
        await reporter.result(testMsg);
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
