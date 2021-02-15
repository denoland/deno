// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { setExitHandler, exit } = window.__bootstrap.os;
  const { exposeForTest } = window.__bootstrap.internals;
  const { metrics } = window.__bootstrap.metrics;
  const { assert } = window.__bootstrap.util;

  // Wrap test function in additional assertion that makes sure
  // the test case does not leak async "ops" - ie. number of async
  // completed ops after the test is the same as number of dispatched
  // ops. Note that "unref" ops are ignored since in nature that are
  // optional.
  function assertOps(fn) {
    return async function asyncOpSanitizer() {
      const pre = metrics();
      try {
        await fn();
      } finally {
        await new Promise((resolve) => setTimeout(resolve, 0));
      }

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

  // Wrap test function in additional assertion that makes sure
  // that the test case does not accidentally exit prematurely.
  function assertExit(fn) {
    return async function exitSanitizer() {
      setExitHandler((exitCode) => {
        assert(
          false,
          `Test case attempted to exit with exit code: ${exitCode}`,
        );
      });

      try {
        await fn();
      } catch (err) {
        throw err;
      } finally {
        setExitHandler(null);
      }
    };
  }

  const tests = [];

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
      sanitizeExit: true,
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

    if (testDef.sanitizeExit) {
      testDef.fn = assertExit(testDef.fn);
    }

    tests.push(testDef);
  }

  function reportTestPlan(plan) {
    core.jsonOpSync("op_report_test_plan", plan);
  }

  function reportTestResult(result) {
    core.jsonOpSync("op_report_test_result", result);
  }

  function reportTestSummary(summary) {
    core.jsonOpSync("op_report_test_summary", summary);
  }

  async function runTests({} = {}) {
    const summary = {
      failed: 0,
      ignored: 0,
      measured: 0,
      passed: 0,
      filtered: 0,
    };

    reportTestPlan({
      count: tests.length,
    });

    for (const { name, ignore, fn } of tests) {
      try {
        if (!ignore) {
          await fn();
        }

        reportTestResult({
          name,
          ignore,
        });

        if (ignore) {
          summary.ignored++;
        } else {
          summary.passed++;
        }
      } catch (error) {
        reportTestResult({
          name,
          error,
        });

        summary.failed++;
      }
    }

    reportTestSummary(summary);
  }

  exposeForTest("runTests", runTests);

  window.__bootstrap.testing = {
    test,
  };
})(this);
