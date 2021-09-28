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
    Error,
    Function,
    JSONStringify,
    Promise,
    TypeError,
    StringPrototypeStartsWith,
    StringPrototypeEndsWith,
    StringPrototypeIncludes,
    StringPrototypeSlice,
    RegExp,
    RegExpPrototypeTest,
    SymbolToStringTag,
  } = window.__bootstrap.primordials;

  // Wrap test function in additional assertion that makes sure
  // the test case does not leak async "ops" - ie. number of async
  // completed ops after the test is the same as number of dispatched
  // ops. Note that "unref" ops are ignored since in nature that are
  // optional.
  function assertOps(fn) {
    return async function asyncOpSanitizer(...params) {
      const pre = metrics();
      try {
        await fn(...params);
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
    return async function resourceSanitizer(...params) {
      const pre = core.resources();
      await fn(...params);
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
    return async function exitSanitizer(...params) {
      setExitHandler((exitCode) => {
        assert(
          false,
          `Test case attempted to exit with exit code: ${exitCode}`,
        );
      });

      try {
        await fn(...params);
      } catch (err) {
        throw err;
      } finally {
        setExitHandler(null);
      }
    };
  }

  function assertTestStepScopes(fn) {
    /** @param step {TestStep} */
    return async function testStepSanitizer(step) {
      await fn(createTester(step));

      const errorMessage = checkStepScopeError();
      if (errorMessage) {
        throw new Error(errorMessage);
      }

      function checkStepScopeError() {
        // check for any running steps
        const hasRunningSteps = ArrayPrototypeSome(
          step.children,
          (r) => r.status === "pending",
        );
        if (hasRunningSteps) {
          return "There were still test steps running after the current scope finished execution. " +
            "Ensure all steps are awaited (ex. `await t.step(...)`).";
        }

        // check if a parent already completed
        let parent = step.parent;
        while (parent) {
          if (parent.finalized) {
            return "Parent scope completed before test step finished execution. " +
              "Ensure all steps are awaited (ex. `await t.step(...)`).";
          }
          parent = parent.parent;
        }

        return undefined;
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

    return async function applyPermissions(...params) {
      const token = pledgePermissions(permissions);

      try {
        await fn(...params);
      } finally {
        restorePermissions(token);
      }
    };
  }

  const tests = [];

  // Main test function provided by Deno.
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

    const step = new TestStep({
      name: test.name,
      parent: undefined,
      sanitizeOps: test.sanitizeOps,
      sanitizeResources: test.sanitizeResources,
      sanitizeExit: test.sanitizeExit,
    });

    let status;
    try {
      await test.fn(step);
      const failCount = step.failedChildStepsCount();
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
      steps: getTestStepResults(step),
    };

    /** @param step {TestStep} */
    function getTestStepResults(step) {
      return ArrayPrototypeMap(
        step.children,
        /** @param childStep {TestStep} */
        (childStep) => ({
          name: childStep.name,
          status: childStep.status,
          steps: getTestStepResults(childStep),
          duration: childStep.duration,
          error: childStep.error,
        }),
      );
    }
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
   *
   * @typedef {{
   *   name: string;
   *   parent: TestStep | undefined,
   *   sanitizeOps: boolean,
   *   sanitizeResources: boolean,
   *   sanitizeExit: boolean,
   * }} TestStepParams
   */

  class TestStep {
    /** @type {TestStepParams} */
    #params;
    finalized = false;
    duration = 0;
    status = "pending";
    error = undefined;
    /** @type {TestStep[]} */
    children = [];

    /** @param params {TestStepParams} */
    constructor(params) {
      this.#params = params;
    }

    get name() {
      return this.#params.name;
    }

    get parent() {
      return this.#params.parent;
    }

    get sanitizerOptions() {
      return {
        sanitizeResources: this.#params.sanitizeResources,
        sanitizeOps: this.#params.sanitizeOps,
        sanitizeExit: this.#params.sanitizeExit,
      };
    }

    failedChildStepsCount() {
      return ArrayPrototypeFilter(
        this.children,
        /** @param step {TestStep} */
        (step) => step.status === "failed",
      ).length;
    }

    usesSanitizer() {
      return this.#params.sanitizeResources ||
        this.#params.sanitizeOps ||
        this.#params.sanitizeExit;
    }

    getFullName() {
      if (this.parent) {
        return `${this.parent.getFullName()} > ${this.name}`;
      } else {
        return this.name;
      }
    }
  }

  function createTester(parentStep) {
    return {
      [SymbolToStringTag]: "Tester",
      /**
       * @param nameOrTestDefinition {string | TestStepDefinition}
       * @param fn {(t: Tester) => void | Promise<void>}
       */
      async step(nameOrTestDefinition, fn) {
        if (parentStep.finalized) {
          throw new Error(
            "Cannot run test step after tester's scope has finished execution. " +
              "Ensure any `.step(...)` calls are executed before their parent scope completes execution.",
          );
        }

        const definition = getDefinition();
        const subStep = new TestStep({
          name: definition.name,
          parent: parentStep,
          sanitizeOps: getOrDefault(
            definition.sanitizeOps,
            parentStep.sanitizerOptions.sanitizeOps,
          ),
          sanitizeResources: getOrDefault(
            definition.sanitizeResources,
            parentStep.sanitizerOptions.sanitizeResources,
          ),
          sanitizeExit: getOrDefault(
            definition.sanitizeExit,
            parentStep.sanitizerOptions.sanitizeExit,
          ),
        });

        ArrayPrototypePush(parentStep.children, subStep);

        if (definition.ignore) {
          subStep.status = "ignored";
          subStep.finalized = true;
          return false;
        }

        const errorMessage = getCannotRunErrorMessage(subStep);
        if (errorMessage) {
          subStep.status = "failed";
          subStep.error = inspectArgs([new Error(errorMessage)]);
          subStep.finalized = true;
          return false;
        }

        const testFn = wrapTestFnWithSanitizers(
          definition.fn,
          subStep.sanitizerOptions,
        );
        const start = DateNow();

        try {
          await testFn(subStep);

          if (subStep.failedChildStepsCount() > 0) {
            subStep.status = "failed";
          } else {
            subStep.status = "ok";
          }
        } catch (error) {
          subStep.error = inspectArgs([error]);
          subStep.status = "failed";
        }

        subStep.duration = DateNow() - start;

        if (subStep.parent?.finalized) {
          // always point this test out as one that was still running
          // if the parent step finalized
          subStep.status = "pending";
        }

        subStep.finalized = true;

        return subStep.status === "ok";

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
      },
    };

    /** @param step {TestStep} */
    function getCannotRunErrorMessage(step) {
      const runningSteps = getPotentialConflictingRunningSteps(step);
      const runningStepsWithSanitizers = ArrayPrototypeFilter(
        runningSteps,
        (t) => t.usesSanitizer(),
      );

      if (runningStepsWithSanitizers.length > 0) {
        return "Cannot start test step while another test step with sanitizers is running.\n" +
          runningStepsWithSanitizers
            .map((s) => ` * ${s.getFullName()}`)
            .join("\n");
      }

      if (step.usesSanitizer() && runningSteps.length > 0) {
        return "Cannot start test step with sanitizers while another test step is running.\n" +
          runningSteps.map((s) => ` * ${s.getFullName()}`).join("\n");
      }

      return undefined;
    }

    /** Returns any running test steps in the execution tree that
     * might conflict with this test.
     * @param step {TestStep}
     */
    function getPotentialConflictingRunningSteps(step) {
      /** @type {TestStep[]} */
      const results = [];
      while (step.parent) {
        const parentStep = step.parent;
        for (const siblingStep of parentStep.children) {
          if (siblingStep === step) {
            continue;
          }
          if (!siblingStep.finalized) {
            ArrayPrototypePush(results, siblingStep);
          }
        }
        step = parentStep;
      }
      return results;
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
