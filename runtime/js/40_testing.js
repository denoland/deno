// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const colors = window.__bootstrap.colors;
  const { exit } = window.__bootstrap.os;
  const { Console, inspectArgs } = window.__bootstrap.console;
  const { stdout } = window.__bootstrap.files;
  const { exposeForTest } = window.__bootstrap.internals;
  const { metrics } = window.__bootstrap.metrics;
  const { assert } = window.__bootstrap.util;

  const disabledConsole = new Console(() => {});

  function delay(ms) {
    return new Promise((resolve) => {
      setTimeout(resolve, ms);
    });
  }

  function formatDuration(time = 0) {
    const gray = colors.maybeColor(colors.gray);
    const italic = colors.maybeColor(colors.italic);
    const timeStr = `(${time}ms)`;
    return gray(italic(timeStr));
  }

  // Wrap test function in additional assertion that makes sure
  // the test case does not leak async "ops" - ie. number of async
  // completed ops after the test is the same as number of dispatched
  // ops. Note that "unref" ops are ignored since in nature that are
  // optional.
  function assertOps(fn) {
    return async function asyncOpSanitizer() {
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
finishing test case.`,
      );
    };
  }

  // Wrap test function in additional assertion that makes sure
  // the test case does not "leak" resources - ie. resource table after
  // the test has exactly the same contents as before the test.
  function assertResources(
    fn,
  ) {
    return async function resourceSanitizer() {
      const pre = core.resources();
      await fn();
      const post = core.resources();

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

  const TEST_REGISTRY = [];

  // Main test function provided by Deno, as you can see it merely
  // creates a new object with "name" and "fn" fields.
  function test(
    t,
    fn,
  ) {
    let testDef;
    const defaults = {
      ignore: false,
      only: false,
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
      testDef = { fn: fn, name: t, ...defaults };
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

  const encoder = new TextEncoder();

  function log(msg, noNewLine = false) {
    if (!noNewLine) {
      msg += "\n";
    }

    // Using `stdout` here because it doesn't force new lines
    // compared to `console.log`; `core.print` on the other hand
    // is line-buffered and doesn't output message without newline
    stdout.writeSync(encoder.encode(msg));
  }

  function reportToConsole(message) {
    const green = colors.maybeColor(colors.green);
    const red = colors.maybeColor(colors.red);
    const yellow = colors.maybeColor(colors.yellow);
    const redFailed = red("FAILED");
    const greenOk = green("ok");
    const yellowIgnored = yellow("ignored");
    if (message.start != null) {
      log(`running ${message.start.tests.length} tests`);
    } else if (message.testStart != null) {
      const { name } = message.testStart;

      log(`test ${name} ... `, true);
      return;
    } else if (message.testEnd != null) {
      switch (message.testEnd.status) {
        case "passed":
          log(`${greenOk} ${formatDuration(message.testEnd.duration)}`);
          break;
        case "failed":
          log(`${redFailed} ${formatDuration(message.testEnd.duration)}`);
          break;
        case "ignored":
          log(`${yellowIgnored} ${formatDuration(message.testEnd.duration)}`);
          break;
      }
    } else if (message.end != null) {
      const failures = message.end.results.filter((m) => m.error != null);
      if (failures.length > 0) {
        log(`\nfailures:\n`);

        for (const { name, error } of failures) {
          log(name);
          log(inspectArgs([error]));
          log("");
        }

        log(`failures:\n`);

        for (const { name } of failures) {
          log(`\t${name}`);
        }
      }
      log(
        `\ntest result: ${message.end.failed ? redFailed : greenOk}. ` +
          `${message.end.passed} passed; ${message.end.failed} failed; ` +
          `${message.end.ignored} ignored; ${message.end.measured} measured; ` +
          `${message.end.filtered} filtered out ` +
          `${formatDuration(message.end.duration)}\n`,
      );

      if (message.end.usedOnly && message.end.failed == 0) {
        log(`${redFailed} because the "only" option was used\n`);
      }
    }
  }

  exposeForTest("reportToConsole", reportToConsole);

  // TODO: already implements AsyncGenerator<RunTestsMessage>, but add as "implements to class"
  // TODO: implements PromiseLike<RunTestsEndResult>
  class TestRunner {
    #usedOnly = false;

    constructor(
      tests,
      filterFn,
      failFast,
    ) {
      this.stats = {
        filtered: 0,
        ignored: 0,
        measured: 0,
        passed: 0,
        failed: 0,
      };
      this.filterFn = filterFn;
      this.failFast = failFast;
      const onlyTests = tests.filter(({ only }) => only);
      this.#usedOnly = onlyTests.length > 0;
      const unfilteredTests = this.#usedOnly ? onlyTests : tests;
      this.testsToRun = unfilteredTests.filter(filterFn);
      this.stats.filtered = unfilteredTests.length - this.testsToRun.length;
    }

    async *[Symbol.asyncIterator]() {
      yield { start: { tests: this.testsToRun } };

      const results = [];
      const suiteStart = +new Date();
      for (const test of this.testsToRun) {
        const endMessage = {
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
        results.push(endMessage);
        yield { testEnd: endMessage };
        if (this.failFast && endMessage.error != null) {
          break;
        }
      }

      const duration = +new Date() - suiteStart;

      yield {
        end: { ...this.stats, usedOnly: this.#usedOnly, duration, results },
      };
    }
  }

  function createFilterFn(
    filter,
    skip,
  ) {
    return (def) => {
      let passes = true;

      if (filter) {
        if (filter instanceof RegExp) {
          passes = passes && filter.test(def.name);
        } else if (filter.startsWith("/") && filter.endsWith("/")) {
          const filterAsRegex = new RegExp(filter.slice(1, filter.length - 1));
          passes = passes && filterAsRegex.test(def.name);
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

  exposeForTest("createFilterFn", createFilterFn);

  async function runTests({
    exitOnFail = true,
    failFast = false,
    filter = undefined,
    skip = undefined,
    disableLog = false,
    reportToConsole: reportToConsole_ = true,
    onMessage = undefined,
  } = {}) {
    const filterFn = createFilterFn(filter, skip);
    const testRunner = new TestRunner(TEST_REGISTRY, filterFn, failFast);

    const originalConsole = globalThis.console;

    if (disableLog) {
      globalThis.console = disabledConsole;
    }

    let endMsg;

    for await (const message of testRunner) {
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

    if ((endMsg.failed > 0 || endMsg?.usedOnly) && exitOnFail) {
      exit(1);
    }

    return endMsg;
  }

  exposeForTest("runTests", runTests);

  window.__bootstrap.testing = {
    test,
  };
})(this);
