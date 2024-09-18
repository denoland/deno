// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { escapeName, withPermissions } from "ext:cli/40_test_common.js";

// TODO(mmastrac): We cannot import these from "ext:core/ops" yet
const {
  op_register_test_step,
  op_register_test,
  op_test_event_step_result_failed,
  op_test_event_step_result_ignored,
  op_test_event_step_result_ok,
  op_test_event_step_wait,
  op_test_get_origin,
} = core.ops;
const {
  ArrayPrototypeFilter,
  ArrayPrototypePush,
  DateNow,
  Error,
  Map,
  MapPrototypeGet,
  MapPrototypeSet,
  SafeArrayIterator,
  SymbolToStringTag,
  TypeError,
} = primordials;

import { setExitHandler } from "ext:runtime/30_os.js";

// Capture `Deno` global so that users deleting or mangling it, won't
// have impact on our sanitizers.
const DenoNs = globalThis.Deno;

/**
 * @typedef {{
 *   id: number,
 *   name: string,
 *   fn: TestFunction
 *   origin: string,
 *   location: TestLocation,
 *   ignore: boolean,
 *   only: boolean.
 *   sanitizeOps: boolean,
 *   sanitizeResources: boolean,
 *   sanitizeExit: boolean,
 *   permissions: PermissionOptions,
 * }} TestDescription
 *
 * @typedef {{
 *   id: number,
 *   name: string,
 *   fn: TestFunction
 *   origin: string,
 *   location: TestLocation,
 *   ignore: boolean,
 *   level: number,
 *   parent: TestDescription | TestStepDescription,
 *   rootId: number,
 *   rootName: String,
 *   sanitizeOps: boolean,
 *   sanitizeResources: boolean,
 *   sanitizeExit: boolean,
 * }} TestStepDescription
 *
 * @typedef {{
 *   context: TestContext,
 *   children: TestStepDescription[],
 *   completed: boolean,
 * }} TestState
 *
 * @typedef {{
 *   context: TestContext,
 *   children: TestStepDescription[],
 *   completed: boolean,
 *   failed: boolean,
 * }} TestStepState
 *
 * @typedef {{
 *   id: number,
 *   name: string,
 *   fn: BenchFunction
 *   origin: string,
 *   ignore: boolean,
 *   only: boolean.
 *   sanitizeExit: boolean,
 *   permissions: PermissionOptions,
 * }} BenchDescription
 */

/** @type {Map<number, TestState | TestStepState>} */
const testStates = new Map();

// Wrap test function in additional assertion that makes sure
// that the test case does not accidentally exit prematurely.
function assertExit(fn, isTest) {
  return async function exitSanitizer(...params) {
    setExitHandler((exitCode) => {
      throw new Error(
        `${
          isTest ? "Test case" : "Bench"
        } attempted to exit with exit code: ${exitCode}`,
      );
    });

    try {
      const innerResult = await fn(...new SafeArrayIterator(params));
      const exitCode = DenoNs.exitCode;
      if (exitCode !== 0) {
        // Reset the code to allow other tests to run...
        DenoNs.exitCode = 0;
        // ...and fail the current test.
        throw new Error(
          `${
            isTest ? "Test case" : "Bench"
          } finished with exit code set to ${exitCode}`,
        );
      }
      if (innerResult) {
        return innerResult;
      }
    } finally {
      setExitHandler(null);
    }
  };
}

function wrapOuter(fn, desc) {
  return async function outerWrapped() {
    try {
      if (desc.ignore) {
        return "ignored";
      }
      return await fn(desc) ?? "ok";
    } catch (error) {
      return { failed: { jsError: core.destructureError(error) } };
    } finally {
      const state = MapPrototypeGet(testStates, desc.id);
      for (const childDesc of state.children) {
        stepReportResult(childDesc, { failed: "incomplete" }, 0);
      }
      state.completed = true;
    }
  };
}

function wrapInner(fn) {
  /** @param desc {TestDescription | TestStepDescription} */
  return async function innerWrapped(desc) {
    function getRunningStepDescs() {
      const results = [];
      let childDesc = desc;
      while (childDesc.parent != null) {
        const state = MapPrototypeGet(testStates, childDesc.parent.id);
        for (const siblingDesc of state.children) {
          if (siblingDesc.id == childDesc.id) {
            continue;
          }
          const siblingState = MapPrototypeGet(testStates, siblingDesc.id);
          if (!siblingState.completed) {
            ArrayPrototypePush(results, siblingDesc);
          }
        }
        childDesc = childDesc.parent;
      }
      return results;
    }
    const runningStepDescs = getRunningStepDescs();
    const runningStepDescsWithSanitizers = ArrayPrototypeFilter(
      runningStepDescs,
      (d) => usesSanitizer(d),
    );

    if (runningStepDescsWithSanitizers.length > 0) {
      return {
        failed: {
          overlapsWithSanitizers: runningStepDescsWithSanitizers.map(
            getFullName,
          ),
        },
      };
    }

    if (usesSanitizer(desc) && runningStepDescs.length > 0) {
      return {
        failed: {
          hasSanitizersAndOverlaps: runningStepDescs.map(getFullName),
        },
      };
    }
    await fn(MapPrototypeGet(testStates, desc.id).context);
    let failedSteps = 0;
    for (const childDesc of MapPrototypeGet(testStates, desc.id).children) {
      const state = MapPrototypeGet(testStates, childDesc.id);
      if (!state.completed) {
        return { failed: "incompleteSteps" };
      }
      if (state.failed) {
        failedSteps++;
      }
    }
    return failedSteps == 0 ? null : { failed: { failedSteps } };
  };
}

const registerTestIdRetBuf = new Uint32Array(1);
const registerTestIdRetBufU8 = new Uint8Array(registerTestIdRetBuf.buffer);

// As long as we're using one isolate per test, we can cache the origin since it won't change
let cachedOrigin = undefined;

function testInner(
  nameOrFnOrOptions,
  optionsOrFn,
  maybeFn,
  overrides = { __proto__: null },
) {
  // No-op if we're not running in `deno test` subcommand.
  if (typeof op_register_test !== "function") {
    return;
  }

  let testDesc;
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
      testDesc = { fn: optionsOrFn, name: nameOrFnOrOptions, ...defaults };
    } else {
      if (!maybeFn || typeof maybeFn !== "function") {
        throw new TypeError("Missing test function");
      }
      if (optionsOrFn.fn != undefined) {
        throw new TypeError(
          "Unexpected 'fn' field in options, test function is already provided as the third argument",
        );
      }
      if (optionsOrFn.name != undefined) {
        throw new TypeError(
          "Unexpected 'name' field in options, test name is already provided as the first argument",
        );
      }
      testDesc = {
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
    testDesc = {
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
          "Unexpected 'fn' field in options, test function is already provided as the second argument",
        );
      }
      name = nameOrFnOrOptions.name ?? fn.name;
    } else {
      if (
        !nameOrFnOrOptions.fn || typeof nameOrFnOrOptions.fn !== "function"
      ) {
        throw new TypeError(
          "Expected 'fn' field in the first argument to be a test function",
        );
      }
      fn = nameOrFnOrOptions.fn;
      name = nameOrFnOrOptions.name ?? fn.name;
    }
    if (!name) {
      throw new TypeError("The test name can't be empty");
    }
    testDesc = { ...defaults, ...nameOrFnOrOptions, fn, name };
  }

  testDesc = { ...testDesc, ...overrides };

  // Delete this prop in case the user passed it. It's used to detect steps.
  delete testDesc.parent;

  if (cachedOrigin == undefined) {
    cachedOrigin = op_test_get_origin();
  }

  testDesc.location = core.currentUserCallSite();
  testDesc.fn = wrapTest(testDesc);
  testDesc.name = escapeName(testDesc.name);

  op_register_test(
    testDesc.fn,
    testDesc.name,
    testDesc.ignore,
    testDesc.only,
    testDesc.sanitizeOps,
    testDesc.sanitizeResources,
    testDesc.location.fileName,
    testDesc.location.lineNumber,
    testDesc.location.columnNumber,
    registerTestIdRetBufU8,
  );
  testDesc.id = registerTestIdRetBuf[0];
  testDesc.origin = cachedOrigin;
  MapPrototypeSet(testStates, testDesc.id, {
    context: createTestContext(testDesc),
    children: [],
    completed: false,
  });
}

// Main test function provided by Deno.
function test(
  nameOrFnOrOptions,
  optionsOrFn,
  maybeFn,
) {
  return testInner(nameOrFnOrOptions, optionsOrFn, maybeFn);
}

test.ignore = function (nameOrFnOrOptions, optionsOrFn, maybeFn) {
  return testInner(nameOrFnOrOptions, optionsOrFn, maybeFn, { ignore: true });
};

test.only = function (
  nameOrFnOrOptions,
  optionsOrFn,
  maybeFn,
) {
  return testInner(nameOrFnOrOptions, optionsOrFn, maybeFn, { only: true });
};

function getFullName(desc) {
  if ("parent" in desc) {
    return `${getFullName(desc.parent)} ... ${desc.name}`;
  }
  return desc.name;
}

function usesSanitizer(desc) {
  return desc.sanitizeResources || desc.sanitizeOps || desc.sanitizeExit;
}

function stepReportResult(desc, result, elapsed) {
  const state = MapPrototypeGet(testStates, desc.id);
  for (const childDesc of state.children) {
    stepReportResult(childDesc, { failed: "incomplete" }, 0);
  }
  if (result === "ok") {
    op_test_event_step_result_ok(desc.id, elapsed);
  } else if (result === "ignored") {
    op_test_event_step_result_ignored(desc.id, elapsed);
  } else {
    op_test_event_step_result_failed(desc.id, result.failed, elapsed);
  }
}

/** @param desc {TestDescription | TestStepDescription} */
function createTestContext(desc) {
  let parent;
  let level;
  let rootId;
  let rootName;
  if ("parent" in desc) {
    parent = MapPrototypeGet(testStates, desc.parent.id).context;
    level = desc.level;
    rootId = desc.rootId;
    rootName = desc.rootName;
  } else {
    parent = undefined;
    level = 0;
    rootId = desc.id;
    rootName = desc.name;
  }
  return {
    [SymbolToStringTag]: "TestContext",
    /**
     * The current test name.
     */
    name: desc.name,
    /**
     * Parent test context.
     */
    parent,
    /**
     * File Uri of the test code.
     */
    origin: desc.origin,
    /**
     * @param nameOrFnOrOptions {string | TestStepDefinition | ((t: TestContext) => void | Promise<void>)}
     * @param maybeFn {((t: TestContext) => void | Promise<void>) | undefined}
     */
    async step(nameOrFnOrOptions, maybeFn) {
      if (MapPrototypeGet(testStates, desc.id).completed) {
        throw new Error(
          "Cannot run test step after parent scope has finished execution. " +
            "Ensure any `.step(...)` calls are executed before their parent scope completes execution.",
        );
      }

      let stepDesc;
      if (typeof nameOrFnOrOptions === "string") {
        if (typeof maybeFn !== "function") {
          throw new TypeError("Expected function for second argument");
        }
        stepDesc = {
          name: nameOrFnOrOptions,
          fn: maybeFn,
        };
      } else if (typeof nameOrFnOrOptions === "function") {
        if (!nameOrFnOrOptions.name) {
          throw new TypeError("The step function must have a name");
        }
        if (maybeFn != undefined) {
          throw new TypeError(
            "Unexpected second argument to TestContext.step()",
          );
        }
        stepDesc = {
          name: nameOrFnOrOptions.name,
          fn: nameOrFnOrOptions,
        };
      } else if (typeof nameOrFnOrOptions === "object") {
        stepDesc = nameOrFnOrOptions;
      } else {
        throw new TypeError(
          "Expected a test definition or name and function",
        );
      }
      stepDesc.ignore ??= false;
      stepDesc.sanitizeOps ??= desc.sanitizeOps;
      stepDesc.sanitizeResources ??= desc.sanitizeResources;
      stepDesc.sanitizeExit ??= desc.sanitizeExit;
      stepDesc.location = core.currentUserCallSite();
      stepDesc.level = level + 1;
      stepDesc.parent = desc;
      stepDesc.rootId = rootId;
      stepDesc.name = escapeName(stepDesc.name);
      stepDesc.rootName = escapeName(rootName);
      stepDesc.fn = wrapTest(stepDesc);
      const id = op_register_test_step(
        stepDesc.name,
        stepDesc.location.fileName,
        stepDesc.location.lineNumber,
        stepDesc.location.columnNumber,
        stepDesc.level,
        stepDesc.parent.id,
        stepDesc.rootId,
        stepDesc.rootName,
      );
      stepDesc.id = id;
      stepDesc.origin = desc.origin;
      const state = {
        context: createTestContext(stepDesc),
        children: [],
        failed: false,
        completed: false,
      };
      MapPrototypeSet(testStates, stepDesc.id, state);
      ArrayPrototypePush(
        MapPrototypeGet(testStates, stepDesc.parent.id).children,
        stepDesc,
      );

      op_test_event_step_wait(stepDesc.id);
      const earlier = DateNow();
      const result = await stepDesc.fn(stepDesc);
      const elapsed = DateNow() - earlier;
      state.failed = !!result.failed;
      stepReportResult(stepDesc, result, elapsed);
      return result == "ok";
    },
  };
}

/**
 * Wrap a user test function in one which returns a structured result.
 * @template T {Function}
 * @param testFn {T}
 * @param desc {TestDescription | TestStepDescription}
 * @returns {T}
 */
function wrapTest(desc) {
  let testFn = wrapInner(desc.fn);
  if (desc.sanitizeExit) {
    testFn = assertExit(testFn, true);
  }
  if (!("parent" in desc) && desc.permissions) {
    testFn = withPermissions(testFn, desc.permissions);
  }
  return wrapOuter(testFn, desc);
}

globalThis.Deno.test = test;
