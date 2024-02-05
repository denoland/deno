// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { escapeName, withPermissions } from "ext:cli/40_test_common.js";

// TODO(mmastrac): We cannot import these from "ext:core/ops" yet
const {
  op_op_names,
  op_register_test_step,
  op_register_test,
  op_test_event_step_result_failed,
  op_test_event_step_result_ignored,
  op_test_event_step_result_ok,
  op_test_event_step_wait,
  op_test_op_sanitizer_collect,
  op_test_op_sanitizer_finish,
  op_test_op_sanitizer_get_async_message,
  op_test_op_sanitizer_report,
} = core.ops;
const {
  ArrayPrototypeFilter,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypeShift,
  DateNow,
  Error,
  Map,
  MapPrototypeGet,
  MapPrototypeHas,
  MapPrototypeSet,
  Promise,
  SafeArrayIterator,
  SymbolToStringTag,
  TypeError,
} = primordials;

import { setExitHandler } from "ext:runtime/30_os.js";
import { setTimeout } from "ext:deno_web/02_timers.js";

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

const opSanitizerDelayResolveQueue = [];
let hasSetOpSanitizerDelayMacrotask = false;

// Even if every resource is closed by the end of a test, there can be a delay
// until the pending ops have all finished. This function returns a promise
// that resolves when it's (probably) fine to run the op sanitizer.
//
// This is implemented by adding a macrotask callback that runs after the
// all ready async ops resolve, and the timer macrotask. Using just a macrotask
// callback without delaying is sufficient, because when the macrotask callback
// runs after async op dispatch, we know that all async ops that can currently
// return `Poll::Ready` have done so, and have been dispatched to JS.
//
// Worker ops are an exception to this, because there is no way for the user to
// await shutdown of the worker from the thread calling `worker.terminate()`.
// Because of this, we give extra leeway for worker ops to complete, by waiting
// for a whole millisecond if there are pending worker ops.
function opSanitizerDelay(hasPendingWorkerOps) {
  if (!hasSetOpSanitizerDelayMacrotask) {
    core.setMacrotaskCallback(handleOpSanitizerDelayMacrotask);
    hasSetOpSanitizerDelayMacrotask = true;
  }
  const p = new Promise((resolve) => {
    // Schedule an async op to complete immediately to ensure the macrotask is
    // run. We rely on the fact that enqueueing the resolver callback during the
    // timeout callback will mean that the resolver gets called in the same
    // event loop tick as the timeout callback.
    setTimeout(() => {
      ArrayPrototypePush(opSanitizerDelayResolveQueue, resolve);
    }, hasPendingWorkerOps ? 1 : 0);
  });
  return p;
}

function handleOpSanitizerDelayMacrotask() {
  const resolve = ArrayPrototypeShift(opSanitizerDelayResolveQueue);
  if (resolve) {
    resolve();
    return opSanitizerDelayResolveQueue.length === 0;
  }
  return undefined; // we performed no work, so can skip microtasks checkpoint
}

let opIdHostRecvMessage = -1;
let opIdHostRecvCtrl = -1;
let opNames = null;

function populateOpNames() {
  opNames = op_op_names();
  opIdHostRecvMessage = opNames.indexOf("op_host_recv_message");
  opIdHostRecvCtrl = opNames.indexOf("op_host_recv_ctrl");
}

// Wrap test function in additional assertion that makes sure
// the test case does not leak async "ops" - ie. number of async
// completed ops after the test is the same as number of dispatched
// ops. Note that "unref" ops are ignored since in nature that are
// optional.
function assertOps(fn) {
  /** @param desc {TestDescription | TestStepDescription} */
  return async function asyncOpSanitizer(desc) {
    if (opNames === null) populateOpNames();
    const res = op_test_op_sanitizer_collect(
      desc.id,
      false,
      opIdHostRecvMessage,
      opIdHostRecvCtrl,
    );
    if (res !== 0) {
      await opSanitizerDelay(res === 2);
      op_test_op_sanitizer_collect(
        desc.id,
        true,
        opIdHostRecvMessage,
        opIdHostRecvCtrl,
      );
    }
    const preTraces = new Map(core.opCallTraces);
    let postTraces;
    let report = null;

    try {
      const innerResult = await fn(desc);
      if (innerResult) return innerResult;
    } finally {
      let res = op_test_op_sanitizer_finish(
        desc.id,
        false,
        opIdHostRecvMessage,
        opIdHostRecvCtrl,
      );
      if (res === 1 || res === 2) {
        await opSanitizerDelay(res === 2);
        res = op_test_op_sanitizer_finish(
          desc.id,
          true,
          opIdHostRecvMessage,
          opIdHostRecvCtrl,
        );
      }
      postTraces = new Map(core.opCallTraces);
      if (res === 3) {
        report = op_test_op_sanitizer_report(desc.id);
      }
    }

    if (report === null) return null;

    const details = [];
    for (const opReport of report) {
      const opName = opNames[opReport.id];
      const diff = opReport.diff;

      if (diff > 0) {
        const [name, hint] = op_test_op_sanitizer_get_async_message(opName);
        const count = diff;
        let message = `${count} async operation${
          count === 1 ? "" : "s"
        } to ${name} ${
          count === 1 ? "was" : "were"
        } started in this test, but never completed.`;
        if (hint) {
          message += ` This is often caused by not ${hint}.`;
        }
        const traces = [];
        for (const [id, { opName: traceOpName, stack }] of postTraces) {
          if (traceOpName !== opName) continue;
          if (MapPrototypeHas(preTraces, id)) continue;
          ArrayPrototypePush(traces, stack);
        }
        if (traces.length === 1) {
          message += " The operation was started here:\n";
          message += traces[0];
        } else if (traces.length > 1) {
          message += " The operations were started here:\n";
          message += ArrayPrototypeJoin(traces, "\n\n");
        }
        ArrayPrototypePush(details, message);
      } else if (diff < 0) {
        const [name, hint] = op_test_op_sanitizer_get_async_message(opName);
        const count = -diff;
        let message = `${count} async operation${
          count === 1 ? "" : "s"
        } to ${name} ${
          count === 1 ? "was" : "were"
        } started before this test, but ${
          count === 1 ? "was" : "were"
        } completed during the test. Async operations should not complete in a test if they were not started in that test.`;
        if (hint) {
          message += ` This is often caused by not ${hint}.`;
        }
        const traces = [];
        for (const [id, { opName: traceOpName, stack }] of preTraces) {
          if (opName !== traceOpName) continue;
          if (MapPrototypeHas(postTraces, id)) continue;
          ArrayPrototypePush(traces, stack);
        }
        if (traces.length === 1) {
          message += " The operation was started here:\n";
          message += traces[0];
        } else if (traces.length > 1) {
          message += " The operations were started here:\n";
          message += ArrayPrototypeJoin(traces, "\n\n");
        }
        ArrayPrototypePush(details, message);
      } else {
        throw new Error("unreachable");
      }
    }

    return {
      failed: { leakedOps: [details, core.isOpCallTracingEnabled()] },
    };
  };
}

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
      if (innerResult) return innerResult;
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

function testInner(
  nameOrFnOrOptions,
  optionsOrFn,
  maybeFn,
  overrides = {},
) {
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
          "Unexpected 'fn' field in options, test function is already provided as the third argument.",
        );
      }
      if (optionsOrFn.name != undefined) {
        throw new TypeError(
          "Unexpected 'name' field in options, test name is already provided as the first argument.",
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
    testDesc = { ...defaults, ...nameOrFnOrOptions, fn, name };
  }

  testDesc = { ...testDesc, ...overrides };

  // Delete this prop in case the user passed it. It's used to detect steps.
  delete testDesc.parent;

  testDesc.location = core.currentUserCallSite();
  testDesc.fn = wrapTest(testDesc);
  testDesc.name = escapeName(testDesc.name);

  const origin = op_register_test(
    testDesc.fn,
    testDesc.name,
    testDesc.ignore,
    testDesc.only,
    false, /*testDesc.sanitizeOps*/
    testDesc.sanitizeResources,
    testDesc.location.fileName,
    testDesc.location.lineNumber,
    testDesc.location.columnNumber,
    registerTestIdRetBufU8,
  );
  testDesc.id = registerTestIdRetBuf[0];
  testDesc.origin = origin;
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
          throw new TypeError("Expected function for second argument.");
        }
        stepDesc = {
          name: nameOrFnOrOptions,
          fn: maybeFn,
        };
      } else if (typeof nameOrFnOrOptions === "function") {
        if (!nameOrFnOrOptions.name) {
          throw new TypeError("The step function must have a name.");
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
          "Expected a test definition or name and function.",
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
  if (desc.sanitizeOps) {
    testFn = assertOps(testFn);
  }
  if (desc.sanitizeExit) {
    testFn = assertExit(testFn, true);
  }
  if (!("parent" in desc) && desc.permissions) {
    testFn = withPermissions(testFn, desc.permissions);
  }
  return wrapOuter(testFn, desc);
}

globalThis.Deno.test = test;
