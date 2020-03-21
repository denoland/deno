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
const disabledConsole = new Console((_x: string, _isErr?: boolean): void => {});

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

export interface RunTestsStartMessage {
  tests: TestDefinition[];
}

export interface TestStartMessage {
  test: TestDefinition;
}

export interface TestEndMessage {
  name: string;
  status: "passed" | "failed" | "ignored";
  duration: number;
  error?: Error;
}

export interface RunTestsEndMessage {
  filtered: number;
  ignored: number;
  measured: number;
  passed: number;
  failed: number;
  duration: number;
  errors: Array<[string, Error]>;
}

export interface TestReporter {
  runTestsStart(message: RunTestsStartMessage): Promise<void>;
  testStart(message: TestStartMessage): Promise<void>;
  testEnd(message: TestEndMessage): Promise<void>;
  runTestsEnd(message: RunTestsEndMessage): Promise<void>;
}

export class ConsoleTestReporter implements TestReporter {
  private encoder: TextEncoder;

  constructor() {
    this.encoder = new TextEncoder();
  }

  private log(msg: string, noNewLine = false): void {
    if (!noNewLine) {
      msg += "\n";
    }

    // Using `stdout` here because it doesn't force new lines
    // compared to `console.log`; `core.print` on the other hand
    // is line-buffered and doesn't output message without newline
    stdout.writeSync(this.encoder.encode(msg));
  }

  runTestsStart(message: RunTestsStartMessage): Promise<void> {
    this.log(`running ${message.tests.length} tests`);
    return Promise.resolve();
  }

  testStart(message: TestStartMessage): Promise<void> {
    const {
      test: { name }
    } = message;

    this.log(`test ${name} ... `, true);
    return Promise.resolve();
  }

  testEnd(message: TestEndMessage): Promise<void> {
    switch (message.status) {
      case "passed":
        this.log(`${GREEN_OK} ${formatDuration(message.duration)}`);
        break;
      case "failed":
        this.log(`${RED_FAILED} ${formatDuration(message.duration)}`);
        break;
      case "ignored":
        this.log(`${YELLOW_IGNORED} ${formatDuration(message.duration)}`);
        break;
    }
    return Promise.resolve();
  }

  runTestsEnd(message: RunTestsEndMessage): Promise<void> {
    // Attempting to match the output of Rust's test runner.
    if (message.errors.length > 0) {
      this.log(`\nfailures:\n`);

      for (const [name, error] of message.errors) {
        this.log(name);
        this.log(stringifyArgs([error]));
        this.log("");
      }

      this.log(`failures:\n`);

      for (const [name] of message.errors) {
        this.log(`\t${name}`);
      }
    }

    this.log(
      `\ntest result: ${message.failed ? RED_FAILED : GREEN_OK}. ` +
        `${message.passed} passed; ${message.failed} failed; ` +
        `${message.ignored} ignored; ${message.measured} measured; ` +
        `${message.filtered} filtered out ` +
        `${formatDuration(message.duration)}\n`
    );
    return Promise.resolve();
  }
}

// TODO: already implements AsyncGenerator<RunTestsMessage>, but add as "implements to class"
// TODO: implements PromiseLike<TestsTestResult>
class TestApi {
  readonly testsToRun: TestDefinition[];
  readonly stats = {
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
  }

  async *[Symbol.asyncIterator](): AsyncIterator<
    | ["start", RunTestsStartMessage]
    | ["testStart", TestStartMessage]
    | ["testEnd", TestEndMessage]
    | ["end", RunTestsEndMessage]
  > {
    yield ["start", { tests: this.testsToRun }];

    const errors: Array<[string, Error]> = [];
    const suiteStart = +new Date();
    for (const test of this.testsToRun) {
      const endMessage: Partial<TestEndMessage> = {
        name: test.name,
        duration: 0
      };
      yield ["testStart", { test }];
      if (test.ignore) {
        endMessage.status = "ignored";
        this.stats.ignored++;
      } else {
        const start = +new Date();
        try {
          await test.fn();
          endMessage.status = "passed";
          this.stats.passed++;
        } catch (err) {
          endMessage.status = "failed";
          endMessage.error = err;
          errors.push([test.name, err]);
          this.stats.failed++;
        } finally {
          endMessage.duration = +new Date() - start;
        }
      }
      yield ["testEnd", endMessage as TestEndMessage];
      if (this.failFast && endMessage.error != null) {
        break;
      }
    }

    const duration = +new Date() - suiteStart;

    yield ["end", { ...this.stats, duration, errors }];
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

export interface RunTestsOptions {
  exitOnFail?: boolean;
  failFast?: boolean;
  only?: string | RegExp;
  skip?: string | RegExp;
  disableLog?: boolean;
  reporter?: TestReporter;
}

export async function runTests({
  exitOnFail = true,
  failFast = false,
  only = undefined,
  skip = undefined,
  disableLog = false,
  reporter = new ConsoleTestReporter()
}: RunTestsOptions = {}): Promise<RunTestsEndMessage> {
  const filterFn = createFilterFn(only, skip);
  const testApi = new TestApi(TEST_REGISTRY, filterFn, failFast);

  // @ts-ignore
  const originalConsole = globalThis.console;

  if (disableLog) {
    // @ts-ignore
    globalThis.console = disabledConsole;
  }

  let endMsg: RunTestsEndMessage;

  for await (const e of testApi) {
    switch (e[0]) {
      case "start":
        await reporter.runTestsStart(e[1]);
        continue;
      case "testStart":
        await reporter.testStart(e[1]);
        continue;
      case "testEnd":
        await reporter.testEnd(e[1]);
        continue;
      case "end":
        endMsg = e[1];
        await reporter.runTestsEnd(e[1]);
        continue;
    }
  }

  if (disableLog) {
    // @ts-ignore
    globalThis.console = originalConsole;
  }

  if (endMsg!.failed > 0 && exitOnFail) {
    exit(1);
  }

  return endMsg!;
}
