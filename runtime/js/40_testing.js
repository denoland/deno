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
    ArrayPrototypePush,
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
    return async function resourceSanitizer() {
      const pre = core.resources();
      await fn();
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

    return async function applyPermissions() {
      const token = pledgePermissions(permissions);

      try {
        await fn();
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

    if (testDef.sanitizeOps) {
      testDef.fn = assertOps(testDef.fn);
    }

    if (testDef.sanitizeResources) {
      testDef.fn = assertResources(testDef.fn);
    }

    if (testDef.sanitizeExit) {
      testDef.fn = assertExit(testDef.fn);
    }

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

  async function runTest({ ignore, fn }) {
    if (ignore) {
      return "ignored";
    }

    try {
      await fn();
      return "ok";
    } catch (error) {
      return { "failed": inspectArgs([error]) };
    }
  }

  function getTestOrigin() {
    return core.opSync("op_get_test_origin");
  }

  function dispatchTestEvent(event) {
    return core.opSync("op_dispatch_test_event", event);
  }

  async function runTests({
    disableLog = false,
    filter = null,
    shuffle = null,
  } = {}) {
    const origin = getTestOrigin();
    const originalConsole = globalThis.console;
    if (disableLog) {
      globalThis.console = new Console(() => {});
    }

    const only = ArrayPrototypeFilter(tests, (test) => test.only);
    const filtered = ArrayPrototypeFilter(
      (only.length > 0 ? only : tests),
      createTestFilter(filter),
    );

    dispatchTestEvent({
      plan: {
        origin,
        total: filtered.length,
        filteredOut: tests.length - filtered.length,
        usedOnly: only.length > 0,
      },
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

      dispatchTestEvent({ wait: description });

      const result = await runTest(test);
      const elapsed = DateNow() - earlier;

      dispatchTestEvent({ result: [description, result, elapsed] });
    }

    if (disableLog) {
      globalThis.console = originalConsole;
    }
  }

  window.__bootstrap.internals = {
    ...window.__bootstrap.internals ?? {},
    runTests,
  };

  window.__bootstrap.testing = {
    test,
  };
})(this);
