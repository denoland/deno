// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { setExitHandler } = window.__bootstrap.os;
  const { Console, inspectArgs } = window.__bootstrap.console;
  const { metrics } = core;
  const { serializePermissions } = window.__bootstrap.permissions;
  const { assert } = window.__bootstrap.util;
  const {
    AggregateError,
    ArrayPrototypeFilter,
    ArrayPrototypePush,
    ArrayPrototypeShift,
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
    Number,
    RegExpPrototypeTest,
    SymbolToStringTag,
  } = window.__bootstrap.primordials;
  let testStepsEnabled = false;

  const opSanitizerDelayResolveQueue = [];

  // Even if every resource is closed by the end of a test, there can be a delay
  // until the pending ops have all finished. This function returns a promise
  // that resolves when it's (probably) fine to run the op sanitizer.
  //
  // This is implemented by adding a macrotask callback that runs after the
  // timer macrotasks, so we can guarantee that a currently running interval
  // will have an associated op. An additional `setTimeout` of 0 is needed
  // before that, though, in order to give time for worker message ops to finish
  // (since timeouts of 0 don't queue tasks in the timer queue immediately).
  function opSanitizerDelay() {
    return new Promise((resolve) => {
      setTimeout(() => {
        ArrayPrototypePush(opSanitizerDelayResolveQueue, resolve);
      }, 0);
    });
  }

  function handleOpSanitizerDelayMacrotask() {
    ArrayPrototypeShift(opSanitizerDelayResolveQueue)?.();
    return opSanitizerDelayResolveQueue.length === 0;
  }

  // Wrap test function in additional assertion that makes sure
  // the test case does not leak async "ops" - ie. number of async
  // completed ops after the test is the same as number of dispatched
  // ops. Note that "unref" ops are ignored since in nature that are
  // optional.
  function assertOps(fn) {
    /** @param step {TestStep} */
    return async function asyncOpSanitizer(step) {
      const pre = metrics();
      try {
        await fn(step);
      } finally {
        // Defer until next event loop turn - that way timeouts and intervals
        // cleared can actually be removed from resource table, otherwise
        // false positives may occur (https://github.com/denoland/deno/issues/4591)
        await opSanitizerDelay();
      }

      if (step.shouldSkipSanitizers) {
        return;
      }

      const post = metrics();

      // We're checking diff because one might spawn HTTP server in the background
      // that will be a pending async op before test starts.
      const dispatchedDiff = post.opsDispatchedAsync - pre.opsDispatchedAsync;
      const completedDiff = post.opsCompletedAsync - pre.opsCompletedAsync;

      const details = [];
      for (const key in post.ops) {
        const dispatchedDiff = Number(
          post.ops[key]?.opsDispatchedAsync -
            (pre.ops[key]?.opsDispatchedAsync ?? 0),
        );
        const completedDiff = Number(
          post.ops[key]?.opsCompletedAsync -
            (pre.ops[key]?.opsCompletedAsync ?? 0),
        );

        if (dispatchedDiff !== completedDiff) {
          details.push(`
  ${key}:
    Before:
      - dispatched: ${pre.ops[key]?.opsDispatchedAsync ?? 0}
      - completed: ${pre.ops[key]?.opsCompletedAsync ?? 0}
    After:
      - dispatched: ${post.ops[key].opsDispatchedAsync}
      - completed: ${post.ops[key].opsCompletedAsync}`);
        }
      }

      const message = `Test case is leaking async ops.
Before:
  - dispatched: ${pre.opsDispatchedAsync}
  - completed: ${pre.opsCompletedAsync}
After:
  - dispatched: ${post.opsDispatchedAsync}
  - completed: ${post.opsCompletedAsync}
${details.length > 0 ? "Ops:" + details.join("") : ""}

Make sure to await all promises returned from Deno APIs before
finishing test case.`;

      assert(
        dispatchedDiff === completedDiff,
        message,
      );
    };
  }

  // Wrap test function in additional assertion that makes sure
  // the test case does not "leak" resources - ie. resource table after
  // the test has exactly the same contents as before the test.
  function assertResources(
    fn,
  ) {
    /** @param step {TestStep} */
    return async function resourceSanitizer(step) {
      const pre = core.resources();
      await fn(step);

      if (step.shouldSkipSanitizers) {
        return;
      }

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
      preValidation();
      // only report waiting after pre-validation
      if (step.canStreamReporting()) {
        step.reportWait();
      }
      await fn(createTestContext(step));
      postValidation();

      function preValidation() {
        const runningSteps = getPotentialConflictingRunningSteps();
        const runningStepsWithSanitizers = ArrayPrototypeFilter(
          runningSteps,
          (t) => t.usesSanitizer,
        );

        if (runningStepsWithSanitizers.length > 0) {
          throw new Error(
            "Cannot start test step while another test step with sanitizers is running.\n" +
              runningStepsWithSanitizers
                .map((s) => ` * ${s.getFullName()}`)
                .join("\n"),
          );
        }

        if (step.usesSanitizer && runningSteps.length > 0) {
          throw new Error(
            "Cannot start test step with sanitizers while another test step is running.\n" +
              runningSteps.map((s) => ` * ${s.getFullName()}`).join("\n"),
          );
        }

        function getPotentialConflictingRunningSteps() {
          /** @type {TestStep[]} */
          const results = [];

          let childStep = step;
          for (const ancestor of step.ancestors()) {
            for (const siblingStep of ancestor.children) {
              if (siblingStep === childStep) {
                continue;
              }
              if (!siblingStep.finalized) {
                ArrayPrototypePush(results, siblingStep);
              }
            }
            childStep = ancestor;
          }
          return results;
        }
      }

      function postValidation() {
        // check for any running steps
        if (step.hasRunningChildren) {
          throw new Error(
            "There were still test steps running after the current scope finished execution. " +
              "Ensure all steps are awaited (ex. `await t.step(...)`).",
          );
        }

        // check if an ancestor already completed
        for (const ancestor of step.ancestors()) {
          if (ancestor.finalized) {
            throw new Error(
              "Parent scope completed before test step finished execution. " +
                "Ensure all steps are awaited (ex. `await t.step(...)`).",
            );
          }
        }
      }
    };
  }

  function withPermissions(fn, permissions) {
    function pledgePermissions(permissions) {
      return core.opSync(
        "op_pledge_test_permissions",
        serializePermissions(permissions),
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
    nameOrFnOrOptions,
    optionsOrFn,
    maybeFn,
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

    if (typeof nameOrFnOrOptions === "string") {
      if (!nameOrFnOrOptions) {
        throw new TypeError("The test name can't be empty");
      }
      if (typeof optionsOrFn === "function") {
        testDef = { fn: optionsOrFn, name: nameOrFnOrOptions, ...defaults };
      } else {
        if (!maybeFn || typeof maybeFn !== "function") {
          throw new TypeError("Missing test function");
        }
        if (optionsOrFn.fn != undefined) {
          throw new TypeError(
            "Unexpected 'fn' field in options, test function is already provided as the third argument.",
          );
        }
        if (optionsOrFn.name != undefined) {
          throw new TypeError(
            "Unexpected 'name' field in options, test name is already provided as the first argument.",
          );
        }
        testDef = {
          ...defaults,
          ...optionsOrFn,
          fn: maybeFn,
          name: nameOrFnOrOptions,
        };
      }
    } else if (typeof nameOrFnOrOptions === "function") {
      if (!nameOrFnOrOptions.name) {
        throw new TypeError("The test function must have a name");
      }
      if (optionsOrFn != undefined) {
        throw new TypeError("Unexpected second argument to Deno.test()");
      }
      if (maybeFn != undefined) {
        throw new TypeError("Unexpected third argument to Deno.test()");
      }
      testDef = {
        ...defaults,
        fn: nameOrFnOrOptions,
        name: nameOrFnOrOptions.name,
      };
    } else {
      let fn;
      let name;
      if (typeof optionsOrFn === "function") {
        fn = optionsOrFn;
        if (nameOrFnOrOptions.fn != undefined) {
          throw new TypeError(
            "Unexpected 'fn' field in options, test function is already provided as the second argument.",
          );
        }
        name = nameOrFnOrOptions.name ?? fn.name;
      } else {
        if (
          !nameOrFnOrOptions.fn || typeof nameOrFnOrOptions.fn !== "function"
        ) {
          throw new TypeError(
            "Expected 'fn' field in the first argument to be a test function.",
          );
        }
        fn = nameOrFnOrOptions.fn;
        name = nameOrFnOrOptions.name ?? fn.name;
      }
      if (!name) {
        throw new TypeError("The test name can't be empty");
      }
      testDef = { ...defaults, ...nameOrFnOrOptions, fn, name };
    }

    testDef.fn = wrapTestFnWithSanitizers(testDef.fn, testDef);

    if (testDef.permissions) {
      testDef.fn = withPermissions(
        testDef.fn,
        testDef.permissions,
      );
    }

    ArrayPrototypePush(tests, testDef);
  }

  function formatError(error) {
    if (error instanceof AggregateError) {
      const message = error
        .errors
        .map((error) =>
          inspectArgs([error]).replace(/^(?!\s*$)/gm, " ".repeat(4))
        )
        .join("\n");

      return error.name + "\n" + message + error.stack;
    }

    return inspectArgs([error]);
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

  async function runTest(test, description) {
    if (test.ignore) {
      return "ignored";
    }

    const step = new TestStep({
      name: test.name,
      parent: undefined,
      rootTestDescription: description,
      sanitizeOps: test.sanitizeOps,
      sanitizeResources: test.sanitizeResources,
      sanitizeExit: test.sanitizeExit,
    });

    try {
      await test.fn(step);
      const failCount = step.failedChildStepsCount();
      return failCount === 0 ? "ok" : {
        "failed": formatError(
          new Error(
            `${failCount} test step${failCount === 1 ? "" : "s"} failed.`,
          ),
        ),
      };
    } catch (error) {
      return {
        "failed": formatError(error),
      };
    } finally {
      step.finalized = true;
      // ensure the children report their result
      for (const child of step.children) {
        child.reportResult();
      }
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

  function reportTestStepWait(testDescription) {
    core.opSync("op_dispatch_test_event", {
      stepWait: testDescription,
    });
  }

  function reportTestStepResult(testDescription, result, elapsed) {
    core.opSync("op_dispatch_test_event", {
      stepResult: [testDescription, result, elapsed],
    });
  }

  async function runTests({
    filter = null,
    shuffle = null,
  } = {}) {
    core.setMacrotaskCallback(handleOpSanitizerDelayMacrotask);

    const origin = getTestOrigin();
    const originalConsole = globalThis.console;

    globalThis.console = new Console(reportTestConsoleOutput);

    const only = ArrayPrototypeFilter(tests, (test) => test.only);
    const filtered = ArrayPrototypeFilter(
      only.length > 0 ? only : tests,
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

      const result = await runTest(test, description);
      const elapsed = DateNow() - earlier;

      reportTestResult(description, result, elapsed);
    }

    globalThis.console = originalConsole;
  }

  /**
   * @typedef {{
   *   fn: (t: TestContext) => void | Promise<void>,
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
   *   rootTestDescription: { origin: string; name: string };
   *   sanitizeOps: boolean,
   *   sanitizeResources: boolean,
   *   sanitizeExit: boolean,
   * }} TestStepParams
   */

  class TestStep {
    /** @type {TestStepParams} */
    #params;
    reportedWait = false;
    #reportedResult = false;
    finalized = false;
    elapsed = 0;
    /** @type "ok" | "ignored" | "pending" | "failed" */
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

    get rootTestDescription() {
      return this.#params.rootTestDescription;
    }

    get sanitizerOptions() {
      return {
        sanitizeResources: this.#params.sanitizeResources,
        sanitizeOps: this.#params.sanitizeOps,
        sanitizeExit: this.#params.sanitizeExit,
      };
    }

    get usesSanitizer() {
      return this.#params.sanitizeResources ||
        this.#params.sanitizeOps ||
        this.#params.sanitizeExit;
    }

    /** If a test validation error already occurred then don't bother checking
     * the sanitizers as that will create extra noise.
     */
    get shouldSkipSanitizers() {
      return this.hasRunningChildren || this.parent?.finalized;
    }

    get hasRunningChildren() {
      return ArrayPrototypeSome(
        this.children,
        /** @param step {TestStep} */
        (step) => step.status === "pending",
      );
    }

    failedChildStepsCount() {
      return ArrayPrototypeFilter(
        this.children,
        /** @param step {TestStep} */
        (step) => step.status === "failed",
      ).length;
    }

    canStreamReporting() {
      // there should only ever be one sub step running when running with
      // sanitizers, so we can use this to tell if we can stream reporting
      return this.selfAndAllAncestorsUseSanitizer() &&
        this.children.every((c) => c.usesSanitizer || c.finalized);
    }

    selfAndAllAncestorsUseSanitizer() {
      if (!this.usesSanitizer) {
        return false;
      }

      for (const ancestor of this.ancestors()) {
        if (!ancestor.usesSanitizer) {
          return false;
        }
      }

      return true;
    }

    *ancestors() {
      let ancestor = this.parent;
      while (ancestor) {
        yield ancestor;
        ancestor = ancestor.parent;
      }
    }

    getFullName() {
      if (this.parent) {
        return `${this.parent.getFullName()} > ${this.name}`;
      } else {
        return this.name;
      }
    }

    reportWait() {
      if (this.reportedWait || !this.parent) {
        return;
      }

      reportTestStepWait(this.#getTestStepDescription());

      this.reportedWait = true;
    }

    reportResult() {
      if (this.#reportedResult || !this.parent) {
        return;
      }

      this.reportWait();

      for (const child of this.children) {
        child.reportResult();
      }

      reportTestStepResult(
        this.#getTestStepDescription(),
        this.#getStepResult(),
        this.elapsed,
      );

      this.#reportedResult = true;
    }

    #getStepResult() {
      switch (this.status) {
        case "ok":
          return "ok";
        case "ignored":
          return "ignored";
        case "pending":
          return {
            "pending": this.error && formatError(this.error),
          };
        case "failed":
          return {
            "failed": this.error && formatError(this.error),
          };
        default:
          throw new Error(`Unhandled status: ${this.status}`);
      }
    }

    #getTestStepDescription() {
      return {
        test: this.rootTestDescription,
        name: this.name,
        level: this.#getLevel(),
      };
    }

    #getLevel() {
      let count = 0;
      for (const _ of this.ancestors()) {
        count++;
      }
      return count;
    }
  }

  /** @param parentStep {TestStep} */
  function createTestContext(parentStep) {
    return {
      [SymbolToStringTag]: "TestContext",
      /**
       * @param nameOrTestDefinition {string | TestStepDefinition}
       * @param fn {(t: TestContext) => void | Promise<void>}
       */
      async step(nameOrTestDefinition, fn) {
        if (!testStepsEnabled) {
          throw new Error(
            "Test steps are unstable. The --unstable flag must be provided.",
          );
        }

        if (parentStep.finalized) {
          throw new Error(
            "Cannot run test step after parent scope has finished execution. " +
              "Ensure any `.step(...)` calls are executed before their parent scope completes execution.",
          );
        }

        const definition = getDefinition();
        const subStep = new TestStep({
          name: definition.name,
          parent: parentStep,
          rootTestDescription: parentStep.rootTestDescription,
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

        try {
          if (definition.ignore) {
            subStep.status = "ignored";
            subStep.finalized = true;
            if (subStep.canStreamReporting()) {
              subStep.reportResult();
            }
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
            subStep.error = formatError(error);
            subStep.status = "failed";
          }

          subStep.elapsed = DateNow() - start;

          if (subStep.parent?.finalized) {
            // always point this test out as one that was still running
            // if the parent step finalized
            subStep.status = "pending";
          }

          subStep.finalized = true;

          if (subStep.reportedWait && subStep.canStreamReporting()) {
            subStep.reportResult();
          }

          return subStep.status === "ok";
        } finally {
          if (parentStep.canStreamReporting()) {
            // flush any buffered steps
            for (const parentChild of parentStep.children) {
              parentChild.reportResult();
            }
          }
        }

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

  function enableTestSteps() {
    testStepsEnabled = true;
  }

  window.__bootstrap.internals = {
    ...window.__bootstrap.internals ?? {},
    runTests,
    enableTestSteps,
  };

  window.__bootstrap.testing = {
    test,
  };
})(this);
