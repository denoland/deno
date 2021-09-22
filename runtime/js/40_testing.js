// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { parsePermissions } = window.__bootstrap.worker;
  const { setExitHandler } = window.__bootstrap.os;
  const { Console, inspectArgs } = window.__bootstrap.console;
  const { metrics } = window.__bootstrap.metrics;
  const { assert } = window.__bootstrap.util;
  const {
    ArrayPrototypeFilter,
    ArrayPrototypeMap,
    ArrayPrototypePush,
    ArrayPrototypeSome,
    DateNow,
    JSONStringify,
    Promise,
    TypeError,
    StringPrototypeStartsWith,
    StringPrototypeEndsWith,
    StringPrototypeIncludes,
    StringPrototypeSlice,
    RegExp,
    RegExpPrototypeTest,
  } = window.__bootstrap.primordials;

  const testerGetTestStepResultsSymbol = Symbol();
  const testerGetStepScopeErrorMessageSymbol = Symbol();
  const testerFailedStepsCountSymbol = Symbol();

  // Wrap test function in additional assertion that makes sure
  // the test case does not leak async "ops" - ie. number of async
  // completed ops after the test is the same as number of dispatched
  // ops. Note that "unref" ops are ignored since in nature that are
  // optional.
  function assertOps(fn) {
    return async function asyncOpSanitizer(tester) {
      const pre = metrics();
      try {
        await fn(tester);
      } finally {
        // Defer until next event loop turn - that way timeouts and intervals
        // cleared can actually be removed from resource table, otherwise
        // false positives may occur (https://github.com/denoland/deno/issues/4591)
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
    return async function resourceSanitizer(tester) {
      const pre = core.resources();
      await fn(tester);
      const post = core.resources();

      const preStr = JSONStringify(pre, null, 2);
      const postStr = JSONStringify(post, null, 2);
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
    return async function exitSanitizer(tester) {
      setExitHandler((exitCode) => {
        assert(
          false,
          `Test case attempted to exit with exit code: ${exitCode}`,
        );
      });

      try {
        await fn(tester);
      } catch (err) {
        throw err;
      } finally {
        setExitHandler(null);
      }
    };
  }

  function assertTestStepScopes(fn) {
    /** @param tester {Tester} */
    return async function testStepScopeSanitizer(tester) {
      await fn(tester);

      const errorMessage = tester[testerGetStepScopeErrorMessageSymbol]();
      if (errorMessage) {
        throw new Error(errorMessage);
      }
    };
  }

  function withPermissions(fn, permissions) {
    function pledgePermissions(permissions) {
      return core.opSync(
        "op_pledge_test_permissions",
        parsePermissions(permissions),
      );
    }

    function restorePermissions(token) {
      core.opSync("op_restore_test_permissions", token);
    }

    return async function applyPermissions(...args) {
      const token = pledgePermissions(permissions);

      try {
        await fn(...args);
      } finally {
        restorePermissions(token);
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
      permissions: null,
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

    testDef.fn = wrapTestFnWithSanitizers(testDef.fn, testDef);

    if (testDef.permissions) {
      testDef.fn = withPermissions(
        testDef.fn,
        parsePermissions(testDef.permissions),
      );
    }

    ArrayPrototypePush(tests, testDef);
  }

  function createTestFilter(filter) {
    return (def) => {
      if (filter) {
        if (
          StringPrototypeStartsWith(filter, "/") &&
          StringPrototypeEndsWith(filter, "/")
        ) {
          const regex = new RegExp(
            StringPrototypeSlice(filter, 1, filter.length - 1),
          );
          return RegExpPrototypeTest(regex, def.name);
        }

        return StringPrototypeIncludes(def.name, filter);
      }

      return true;
    };
  }

  async function runTest(test) {
    if (test.ignore) {
      return {
        status: "ignored",
        steps: [],
      };
    }

    const tester = new Tester({
      name: test.name,
      sanitizeOps: test.sanitizeOps,
      sanitizeResources: test.sanitizeResources,
      sanitizeExit: test.sanitizeExit,
      parent: undefined,
    });

    let status;
    try {
      await test.fn(tester);
      const failCount = tester[testerFailedStepsCountSymbol]();
      status = failCount === 0 ? "ok" : {
        "failed": inspectArgs([
          new Error(
            `${failCount} test step${failCount === 1 ? "" : "s"} failed.`,
          ),
        ]),
      };
    } catch (error) {
      status = {
        "failed": inspectArgs([error]),
      };
    }

    return {
      status,
      steps: tester[testerGetTestStepResultsSymbol](),
    };
  }

  function getTestOrigin() {
    return core.opSync("op_get_test_origin");
  }

  function reportTestPlan(plan) {
    core.opSync("op_dispatch_test_event", {
      plan,
    });
  }

  function reportTestConsoleOutput(console) {
    core.opSync("op_dispatch_test_event", {
      output: { console },
    });
  }

  function reportTestWait(test) {
    core.opSync("op_dispatch_test_event", {
      wait: test,
    });
  }

  function reportTestResult(test, result, elapsed) {
    core.opSync("op_dispatch_test_event", {
      result: [test, result, elapsed],
    });
  }

  async function runTests({
    filter = null,
    shuffle = null,
  } = {}) {
    const origin = getTestOrigin();
    const originalConsole = globalThis.console;

    globalThis.console = new Console(reportTestConsoleOutput);

    const only = ArrayPrototypeFilter(tests, (test) => test.only);
    const filtered = ArrayPrototypeFilter(
      (only.length > 0 ? only : tests),
      createTestFilter(filter),
    );

    reportTestPlan({
      origin,
      total: filtered.length,
      filteredOut: tests.length - filtered.length,
      usedOnly: only.length > 0,
    });

    if (shuffle !== null) {
      // http://en.wikipedia.org/wiki/Linear_congruential_generator
      const nextInt = (function (state) {
        const m = 0x80000000;
        const a = 1103515245;
        const c = 12345;

        return function (max) {
          return state = ((a * state + c) % m) % max;
        };
      }(shuffle));

      for (let i = filtered.length - 1; i > 0; i--) {
        const j = nextInt(i);
        [filtered[i], filtered[j]] = [filtered[j], filtered[i]];
      }
    }

    for (const test of filtered) {
      const description = {
        origin,
        name: test.name,
      };
      const earlier = DateNow();

      reportTestWait(description);

      const result = await runTest(test);
      result.duration = DateNow() - earlier;

      reportTestResult(description, result);
    }

    globalThis.console = originalConsole;
  }

  /**
   * @typedef {{
   *   fn: (t: Tester) => void | Promise<void>,
   *   name: string,
   *   ignore?: boolean,
   *   sanitizeOps?: boolean,
   *   sanitizeResources?: boolean,
   *   sanitizeExit?: boolean,
   * }} TestStepDefinition
   */

  /**
   * @typedef {{
   *   name: string;
   *   sanitizeOps: boolean,
   *   sanitizeResources: boolean,
   *   sanitizeExit: boolean,
   *   parent: Tester | undefined,
   * }} TesterParams
   */

  /**
   * @typedef {{
   *   definition: TestStepDefinition,
   *   tester: Tester | undefined,
   *   status: "pending" | "ignored" | "ok" | "failed",
   *   usesSanitizer: boolean;
   *   duration: number,
   *   error: string | undefined,
   * }} TestStatus
   */

  // todo: export this class as `Deno.Tester` I guess...
  class Tester {
    /** @type {string} */
    #name;
    /** @type {Tester | undefined} */
    #parent;
    /** @type {bool} */
    #sanitizeResources;
    /** @type {bool} */
    #sanitizeOps;
    /** @type {bool} */
    #sanitizeExit;
    #finalized = false;
    /** @type {TestStatus[]} */
    #testStatuses = [];

    /** @param params {TesterParams} */
    constructor(params) {
      this.#name = params.name;
      this.#parent = params.parent;
      this.#sanitizeResources = params.sanitizeResources;
      this.#sanitizeOps = params.sanitizeOps;
      this.#sanitizeExit = params.sanitizeExit;
    }

    /**
     * @param nameOrTestDefinition {string | TestStepDefinition}
     * @param fn {(t: Tester) => void | Promise<void>}
     */
    async step(nameOrTestDefinition, fn) {
      if (this.#finalized) {
        throw new Error(
          "Cannot run test step after tester's scope has finished execution. " +
            "Ensure any `.step(...)` calls are executed before their parent scope completes execution.",
        );
      }

      const definition = getDefinition();
      /** @type {TestStatus} */
      const testStatus = {
        definition,
        tester: undefined,
        status: "pending",
        duration: 0,
        error: undefined,
      };
      ArrayPrototypePush(this.#testStatuses, testStatus);

      if (definition.ignore) {
        testStatus.status = "ignored";
        return true;
      }

      /** @type {TesterParams} */
      const subTesterParams = {
        name: definition.name,
        sanitizeOps: getOrDefault(
          definition.sanitizeOps,
          this.#sanitizeOps,
        ),
        sanitizeResources: getOrDefault(
          definition.sanitizeResources,
          this.#sanitizeResources,
        ),
        sanitizeExit: getOrDefault(
          definition.sanitizeExit,
          this.#sanitizeExit,
        ),
        parent: this,
      };

      const tester = new Tester(subTesterParams);
      testStatus.tester = tester;

      const errorMessage = tester.#checkCanRun();
      if (errorMessage) {
        testStatus.status = "failed";
        testStatus.error = inspectArgs([new Error(errorMessage)]);
        return false;
      }

      const testFn = wrapTestFnWithSanitizers(definition.fn, subTesterParams);
      const start = DateNow();

      try {
        await testFn(tester);

        if (tester[testerFailedStepsCountSymbol]() > 0) {
          testStatus.status = "failed";
        } else {
          testStatus.status = "ok";
        }
      } catch (error) {
        testStatus.error = inspectArgs([error]);
        testStatus.status = "failed";
      }

      if (tester.#parent != null && tester.#parent.#finalized) {
        // always point this test out as one that was still running
        // if the parent tester finalized
        testStatus.status = "pending";
      }

      tester.#finalized = true;
      testStatus.duration = DateNow() - start;

      return testStatus.status === "ok";

      /** @returns {TestStepDefinition} */
      function getDefinition() {
        if (typeof nameOrTestDefinition === "string") {
          if (!(fn instanceof Function)) {
            throw new TypeError("Expected function for second argument.");
          }
          return {
            name: nameOrTestDefinition,
            fn,
          };
        } else if (typeof nameOrTestDefinition === "object") {
          return nameOrTestDefinition;
        } else {
          throw new TypeError(
            "Expected a test definition or name and function.",
          );
        }
      }
    }

    [testerGetTestStepResultsSymbol]() {
      return ArrayPrototypeMap(
        this.#testStatuses,
        /** @param status {TestStatus} */
        (status) => ({
          name: status.definition.name,
          status: status.status,
          steps: status.tester?.[testerGetTestStepResultsSymbol]() ?? [],
          duration: status.duration,
          error: status.error,
        }),
      );
    }

    [testerGetStepScopeErrorMessageSymbol]() {
      // check for any running steps
      const hasRunningSteps = ArrayPrototypeSome(
        this.#testStatuses,
        (r) => r.status === "pending",
      );
      if (hasRunningSteps) {
        return "There were still test steps running after the current scope finished execution. " +
          "Ensure all steps are awaited (ex. `await t.step(...)`).";
      }

      // check if a parent already completed
      let parent = this.#parent;
      while (parent != null) {
        if (parent.#finalized) {
          return "Parent scope completed before test step finished execution. " +
            "Ensure all steps are awaited (ex. `await t.step(...)`).";
        }
        parent = parent.#parent;
      }

      return undefined;
    }

    [testerFailedStepsCountSymbol]() {
      return ArrayPrototypeFilter(
        this.#testStatuses,
        /** @param status {TestStatus} */
        (status) => status.status === "failed",
      ).length;
    }

    #checkCanRun() {
      const runningTesters = this.#getNonAncestorRunningTesters();
      const runningTestersWithSanitizers = ArrayPrototypeFilter(
        runningTesters,
        (t) => t.#usesSanitizer(),
      );

      if (runningTestersWithSanitizers.length > 0) {
        return "Cannot start test step while another test step with sanitizers is running.\n" +
          runningTestersWithSanitizers
            .map((t) => ` * ${t.#getFullName()}`)
            .join("\n");
      }

      if (this.#usesSanitizer() && runningTesters.length > 0) {
        return "Cannot start test step with sanitizers while another test step is running.\n" +
          runningTesters.map((t) => ` * ${t.#getFullName()}`).join("\n");
      }

      return undefined;
    }

    /** Checks all the nodes in the tree except this tester's
     * ancestors for any running tests. If found, returns those testers.
     */
    #getNonAncestorRunningTesters() {
      let tester = this;
      /** @type {Tester[]} */
      let results = [];
      while (tester.#parent != null) {
        const parentTester = tester.#parent;
        for (const testStatus of parentTester.#testStatuses) {
          const siblingTester = testStatus.tester;
          if (siblingTester == null || siblingTester === tester) {
            continue;
          }
          if (!siblingTester.#finalized) {
            results.push(siblingTester);
          }
        }
        tester = parentTester;
      }
      return results;
    }

    #usesSanitizer() {
      return this.#sanitizeResources ||
        this.#sanitizeOps ||
        this.#sanitizeExit;
    }

    #getFullName() {
      if (this.#parent != null) {
        return `${this.#parent.#getFullName()} > ${this.#name}`;
      } else {
        return this.#name;
      }
    }
  }

  /**
   * @template T {Function}
   * @param testFn {T}
   * @param opts {{
   *   sanitizeOps: boolean,
   *   sanitizeResources: boolean,
   *   sanitizeExit: boolean,
   * }}
   * @returns {T}
   */
  function wrapTestFnWithSanitizers(testFn, opts) {
    testFn = assertTestStepScopes(testFn);

    if (opts.sanitizeOps) {
      testFn = assertOps(testFn);
    }
    if (opts.sanitizeResources) {
      testFn = assertResources(testFn);
    }
    if (opts.sanitizeExit) {
      testFn = assertExit(testFn);
    }
    return testFn;
  }

  /**
   * @template T
   * @param value {T | undefined}
   * @param defaultValue {T}
   * @returns T
   */
  function getOrDefault(value, defaultValue) {
    return value == null ? defaultValue : value;
  }

  window.__bootstrap.internals = {
    ...window.__bootstrap.internals ?? {},
    runTests,
  };

  window.__bootstrap.testing = {
    test,
  };
})(this);
