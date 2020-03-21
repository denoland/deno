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
  kind: "runTestsStart";
  tests: TestDefinition[];
}

export interface TestStartMessage {
  kind: "testStart";
  test: TestDefinition;
}

export interface TestEndMessage {
  kind: "testEnd";
  name: string;
  status: "passed" | "failed" | "ignored";
  duration: number;
  error?: Error;
}

export interface RunTestsEndMessage {
  kind: "runTestsEnd";
  filtered: number;
  ignored: number;
  measured: number;
  passed: number;
  failed: number;
  duration: number;
  errors: Array<[string, Error]>;
}

export type TestMessage =
  | RunTestsStartMessage
  | TestStartMessage
  | TestEndMessage
  | RunTestsEndMessage;

const encoder = new TextEncoder();

function log(msg: string, noNewLine = false): void {
  if (!noNewLine) {
    msg += "\n";
  }

  // Using `stdout` here because it doesn't force new lines
  // compared to `console.log`; `core.print` on the other hand
  // is line-buffered and doesn't output message without newline
  stdout.writeSync(encoder.encode(msg));
}

export function reportToConsole(message: TestMessage): void {
  if (message.kind == "runTestsStart") {
    log(`running ${message.tests.length} tests`);
  } else if (message.kind == "testStart") {
    const {
      test: { name }
    } = message;

    log(`test ${name} ... `, true);
    return;
  } else if (message.kind == "testEnd") {
    switch (message.status) {
      case "passed":
        log(`${GREEN_OK} ${formatDuration(message.duration)}`);
        break;
      case "failed":
        log(`${RED_FAILED} ${formatDuration(message.duration)}`);
        break;
      case "ignored":
        log(`${YELLOW_IGNORED} ${formatDuration(message.duration)}`);
        break;
    }
  } else if (message.kind == "runTestsEnd") {
    if (message.errors.length > 0) {
      log(`\nfailures:\n`);

      for (const [name, error] of message.errors) {
        log(name);
        log(stringifyArgs([error]));
        log("");
      }

      log(`failures:\n`);

      for (const [name] of message.errors) {
        log(`\t${name}`);
      }
    }
    log(
      `\ntest result: ${message.failed ? RED_FAILED : GREEN_OK}. ` +
        `${message.passed} passed; ${message.failed} failed; ` +
        `${message.ignored} ignored; ${message.measured} measured; ` +
        `${message.filtered} filtered out ` +
        `${formatDuration(message.duration)}\n`
    );
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

  async *[Symbol.asyncIterator](): AsyncIterator<TestMessage> {
    yield { kind: "runTestsStart", tests: this.testsToRun };

    const errors: Array<[string, Error]> = [];
    const suiteStart = +new Date();
    for (const test of this.testsToRun) {
      const endMessage: Partial<TestEndMessage> = {
        kind: "testEnd",
        name: test.name,
        duration: 0
      };
      yield { kind: "testStart", test };
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
      yield endMessage as TestEndMessage;
      if (this.failFast && endMessage.error != null) {
        break;
      }
    }

    const duration = +new Date() - suiteStart;

    yield { kind: "runTestsEnd", ...this.stats, duration, errors };
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
  reportToConsole?: boolean;
}

export async function* runTests({
  exitOnFail = true,
  failFast = false,
  only = undefined,
  skip = undefined,
  disableLog = false,
  reportToConsole: reportToConsole_ = false
}: RunTestsOptions = {}): AsyncIterableIterator<TestMessage> {
  const filterFn = createFilterFn(only, skip);
  const testApi = new TestApi(TEST_REGISTRY, filterFn, failFast);

  // @ts-ignore
  const originalConsole = globalThis.console;

  if (disableLog) {
    // @ts-ignore
    globalThis.console = disabledConsole;
  }

  let endMsg: RunTestsEndMessage;

  for await (const message of testApi) {
    yield message;
    if (reportToConsole_) {
      reportToConsole(message);
    }
    if (message.kind == "runTestsEnd") {
      endMsg = message;
    }
  }

  if (disableLog) {
    // @ts-ignore
    globalThis.console = originalConsole;
  }

  if (endMsg!.failed > 0 && exitOnFail) {
    exit(1);
  }
}
