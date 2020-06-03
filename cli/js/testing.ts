// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { gray, green, italic, red, yellow } from "./colors.ts";
import { exit } from "./ops/os.ts";
import { Console, stringifyArgs } from "./web/console.ts";
import { stdout } from "./files.ts";
import { exposeForTest } from "./internals.ts";
import { TextEncoder } from "./web/text_encoding.ts";
import { metrics } from "./ops/runtime.ts";
import { resources } from "./ops/resources.ts";
import { assert } from "./util.ts";

const RED_FAILED = red("FAILED");
const GREEN_OK = green("ok");
const YELLOW_IGNORED = yellow("ignored");
const disabledConsole = new Console((): void => {});

function delay(n: number): Promise<void> {
  return new Promise((resolve: () => void, _) => {
    setTimeout(resolve, n);
  });
}

function formatDuration(time = 0): string {
  const timeStr = `(${time}ms)`;
  return gray(italic(timeStr));
}

// Wrap test function in additional assertion that makes sure
// the test case does not leak async "ops" - ie. number of async
// completed ops after the test is the same as number of dispatched
// ops. Note that "unref" ops are ignored since in nature that are
// optional.
function assertOps(fn: () => void | Promise<void>): () => void | Promise<void> {
  return async function asyncOpSanitizer(): Promise<void> {
    const pre = metrics();
    await fn();
    // Defer until next event loop turn - that way timeouts and intervals
    // cleared can actually be removed from resource table, otherwise
    // false positives may occur (https://github.com/denoland/deno/issues/4591)
    await delay(0);
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
  - completed: ${post.opsCompletedAsync}
  
Make sure to await all promises returned from Deno APIs before 
finishing test case.`
    );
  };
}

// Wrap test function in additional assertion that makes sure
// the test case does not "leak" resources - ie. resource table after
// the test has exactly the same contents as before the test.
function assertResources(
  fn: () => void | Promise<void>
): () => void | Promise<void> {
  return async function resourceSanitizer(): Promise<void> {
    const pre = resources();
    await fn();
    const post = resources();

    const preStr = JSON.stringify(pre, null, 2);
    const postStr = JSON.stringify(post, null, 2);
    const msg = `Test case is leaking resources.
Before: ${preStr}
After: ${postStr}

Make sure to close all open resource handles returned from Deno APIs before 
finishing test case.`;
    assert(preStr === postStr, msg);
  };
}

export interface TestDefinition {
  fn: () => void | Promise<void>;
  name: string;
  ignore?: boolean;
  sanitizeOps?: boolean;
  sanitizeResources?: boolean;
}

const TEST_REGISTRY: TestDefinition[] = [];

export function test(t: TestDefinition): void;
export function test(name: string, fn: () => void | Promise<void>): void;
// Main test function provided by Deno, as you can see it merely
// creates a new object with "name" and "fn" fields.
export function test(
  t: string | TestDefinition,
  fn?: () => void | Promise<void>
): void {
  let testDef: TestDefinition;
  const defaults = {
    ignore: false,
    sanitizeOps: true,
    sanitizeResources: true,
  };

  if (typeof t === "string") {
    if (!fn || typeof fn != "function") {
      throw new TypeError("Missing test function");
    }
    if (!t) {
      throw new TypeError("The test name can't be empty");
    }
    testDef = { fn: fn as () => void | Promise<void>, name: t, ...defaults };
  } else {
    if (!t.fn) {
      throw new TypeError("Missing test function");
    }
    if (!t.name) {
      throw new TypeError("The test name can't be empty");
    }
    testDef = { ...defaults, ...t };
  }

  if (testDef.sanitizeOps) {
    testDef.fn = assertOps(testDef.fn);
  }

  if (testDef.sanitizeResources) {
    testDef.fn = assertResources(testDef.fn);
  }

  TEST_REGISTRY.push(testDef);
}

interface TestMessage {
  start?: {
    tests: TestDefinition[];
  };
  // Must be extensible, avoiding `testStart?: TestDefinition;`.
  testStart?: {
    [P in keyof TestDefinition]: TestDefinition[P];
  };
  testEnd?: {
    name: string;
    status: "passed" | "failed" | "ignored";
    duration: number;
    error?: Error;
  };
  end?: {
    filtered: number;
    ignored: number;
    measured: number;
    passed: number;
    failed: number;
    duration: number;
    results: Array<TestMessage["testEnd"] & {}>;
  };
}

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

function reportToConsole(message: TestMessage): void {
  if (message.start != null) {
    log(`running ${message.start.tests.length} tests`);
  } else if (message.testStart != null) {
    const { name } = message.testStart;

    log(`test ${name} ... `, true);
    return;
  } else if (message.testEnd != null) {
    switch (message.testEnd.status) {
      case "passed":
        log(`${GREEN_OK} ${formatDuration(message.testEnd.duration)}`);
        break;
      case "failed":
        log(`${RED_FAILED} ${formatDuration(message.testEnd.duration)}`);
        break;
      case "ignored":
        log(`${YELLOW_IGNORED} ${formatDuration(message.testEnd.duration)}`);
        break;
    }
  } else if (message.end != null) {
    const failures = message.end.results.filter((m) => m.error != null);
    if (failures.length > 0) {
      log(`\nfailures:\n`);

      for (const { name, error } of failures) {
        log(name);
        log(stringifyArgs([error!]));
        log("");
      }

      log(`failures:\n`);

      for (const { name } of failures) {
        log(`\t${name}`);
      }
    }
    log(
      `\ntest result: ${message.end.failed ? RED_FAILED : GREEN_OK}. ` +
        `${message.end.passed} passed; ${message.end.failed} failed; ` +
        `${message.end.ignored} ignored; ${message.end.measured} measured; ` +
        `${message.end.filtered} filtered out ` +
        `${formatDuration(message.end.duration)}\n`
    );
  }
}

exposeForTest("reportToConsole", reportToConsole);

// TODO: already implements AsyncGenerator<RunTestsMessage>, but add as "implements to class"
// TODO: implements PromiseLike<RunTestsEndResult>
class TestApi {
  readonly testsToRun: TestDefinition[];
  readonly stats = {
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

  async *[Symbol.asyncIterator](): AsyncIterator<TestMessage> {
    yield { start: { tests: this.testsToRun } };

    const results: Array<TestMessage["testEnd"] & {}> = [];
    const suiteStart = +new Date();
    for (const test of this.testsToRun) {
      const endMessage: Partial<TestMessage["testEnd"] & {}> = {
        name: test.name,
        duration: 0,
      };
      yield { testStart: { ...test } };
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
          this.stats.failed++;
        }
        endMessage.duration = +new Date() - start;
      }
      results.push(endMessage as TestMessage["testEnd"] & {});
      yield { testEnd: endMessage as TestMessage["testEnd"] };
      if (this.failFast && endMessage.error != null) {
        break;
      }
    }

    const duration = +new Date() - suiteStart;

    yield { end: { ...this.stats, duration, results } };
  }
}

function createFilterFn(
  filter: undefined | string | RegExp,
  skip: undefined | string | RegExp
): (def: TestDefinition) => boolean {
  return (def: TestDefinition): boolean => {
    let passes = true;

    if (filter) {
      if (filter instanceof RegExp) {
        passes = passes && filter.test(def.name);
      } else {
        passes = passes && def.name.includes(filter);
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

interface RunTestsOptions {
  exitOnFail?: boolean;
  failFast?: boolean;
  filter?: string | RegExp;
  skip?: string | RegExp;
  disableLog?: boolean;
  reportToConsole?: boolean;
  onMessage?: (message: TestMessage) => void | Promise<void>;
}

async function runTests({
  exitOnFail = true,
  failFast = false,
  filter = undefined,
  skip = undefined,
  disableLog = false,
  reportToConsole: reportToConsole_ = true,
  onMessage = undefined,
}: RunTestsOptions = {}): Promise<TestMessage["end"] & {}> {
  const filterFn = createFilterFn(filter, skip);
  const testApi = new TestApi(TEST_REGISTRY, filterFn, failFast);

  const originalConsole = globalThis.console;

  if (disableLog) {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (globalThis as any).console = disabledConsole;
  }

  let endMsg: TestMessage["end"];

  for await (const message of testApi) {
    if (onMessage != null) {
      await onMessage(message);
    }
    if (reportToConsole_) {
      reportToConsole(message);
    }
    if (message.end != null) {
      endMsg = message.end;
    }
  }

  if (disableLog) {
    globalThis.console = originalConsole;
  }

  if (endMsg!.failed > 0 && exitOnFail) {
    exit(1);
  }

  return endMsg!;
}

exposeForTest("runTests", runTests);
