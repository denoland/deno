// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// @ts-check

import { core, primordials } from "ext:core/mod.js";
import { escapeName, withPermissions } from "ext:cli/40_test_common.js";

// TODO(mmastrac): We cannot import these from "ext:core/ops" yet
const {
  op_register_test_step,
  op_register_test,
  op_test_group_register,
  op_test_group_event_start,
  op_test_group_event_end,
  op_register_test_run_fn,
  op_test_event_step_result_failed,
  op_test_event_step_result_ignored,
  op_test_event_step_result_ok,
  op_test_event_step_wait,
  op_test_event_start,
  op_test_event_result_ok,
  op_test_event_result_ignored,
  op_test_event_result_failed,
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
 *   only: boolean,
 *   sanitizeOps: boolean,
 *   sanitizeResources: boolean,
 *   sanitizeExit: boolean,
 *   permissions: Deno.PermissionOptions,
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
 *   only: boolean,
 *   sanitizeExit: boolean,
 *   permissions: Deno.PermissionOptions,
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
  /** @param  {TestDescription | TestStepDescription} desc */
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

const registerTestGroupIdRetBuf = new Uint32Array(1);
const registerTestGroupIdRetBufU8 = new Uint8Array(
  registerTestGroupIdRetBuf.buffer,
);

/**
 * As long as we're using one isolate per test, we can cache the origin
 * since it won't change.
 * @type {string | undefined}
 */
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
    ROOT_TEST_GROUP.id,
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

/** @param {TestDescription | TestStepDescription} desc */
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
     * @param {string | TestStepDescription | ((t: TestContext) => void | Promise<void>)} nameOrFnOrOptions
     * @param {((t: TestContext) => void | Promise<void>) | undefined} maybeFn
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
 * @template {Function} T
 * @param {TestDescription | TestStepDescription} desc
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

/**
 * @typedef {{
 *   id: number,
 *   parentId: number,
 *   name: string,
 *   fn: () => any,
 *   only: boolean,
 *   ignore: boolean,
 *   location: TestLocationInfo,
 * }} BddTest
 *
 * @typedef {() => unknown | Promise<unknown>} TestLifecycleFn
 *
 * @typedef {{
 *   id: number,
 *   parentId: number,
 *   name: string,
 *   ignore: boolean,
 *   only: boolean,
 *   children: Array<TestGroup | BddTest>,
 *   beforeAll: TestLifecycleFn | null,
 *   afterAll: TestLifecycleFn | null,
 *   beforeEach: TestLifecycleFn | null,
 *   afterEach: TestLifecycleFn | null
 *   sanitizeOps: boolean,
 *   sanitizeResources: boolean,
 *   sanitizeExit: boolean,
 *   permissions?: Deno.PermissionOptions,
 * }} TestGroup
 *
 * @typedef {{
 *   only: boolean,
 *   ignore: boolean,
 *   name: string,
 *   fn: () => any,
 *   sanitizeOps: boolean,
 *   sanitizeResources: boolean,
 *   sanitizeExit: boolean,
 *   permissions?: Deno.PermissionOptions,
 * }} BddArgs
 */

/** @type {TestGroup} */
const ROOT_TEST_GROUP = {
  id: 0,
  parentId: 0,
  name: "__DENO_TEST_ROOT__",
  ignore: false,
  only: false,
  children: [],
  beforeAll: null,
  beforeEach: null,
  afterAll: null,
  afterEach: null,
  sanitizeExit: false,
  sanitizeOps: false,
  sanitizeResources: false,
  permissions: undefined,
};
// No-op if we're not running in `deno test` subcommand.
if (typeof op_register_test === "function") {
  op_test_group_register(
    registerTestGroupIdRetBufU8,
    ROOT_TEST_GROUP.name,
    ROOT_TEST_GROUP.parentId,
  );
  ROOT_TEST_GROUP.id = registerTestGroupIdRetBuf[0];
}

/** @type {{ hasOnly: boolean, stack: TestGroup[], total: number }} */
const BDD_CONTEXT = {
  hasOnly: false,
  stack: [ROOT_TEST_GROUP],
  total: 0,
};

/**
 * @overload
 * @param {() => any} nameOrFnOrOptions
 * @returns {BddArgs}
 */
/**
 * @overload
 * @param {BddArgs} nameOrFnOrOptions
 * @returns {BddArgs}
 */
/**
 * @overload
 * @param {string} nameOrFnOrOptions
 * @param {() => any} fnOrOptions
 * @returns {BddArgs}
 */
/**
 * @overload
 * @param {string} nameOrFnOrOptions
 * @param {BddArgs} fnOrOptions
 * @param {() => any} maybeFn
 * @returns {BddArgs}
 */
/**
 * @param {string | (() => any) | BddArgs} nameOrFnOrOptions
 * @param {(() => any) | BddArgs} [fnOrOptions]
 * @param {(() => any)} [maybeFn]
 * @returns {BddArgs}
 */
function normalizeBddArgs(nameOrFnOrOptions, fnOrOptions, maybeFn) {
  let name = "";
  let fn;
  let only = false;
  let ignore = false;
  let sanitizeExit = false;
  let sanitizeOps = false;
  let sanitizeResources = false;
  let permissions;

  if (typeof nameOrFnOrOptions === "function") {
    name = nameOrFnOrOptions.name;
    fn = nameOrFnOrOptions;
  } else if (typeof nameOrFnOrOptions === "object") {
    return nameOrFnOrOptions;
  } else if (typeof fnOrOptions === "function") {
    name = nameOrFnOrOptions;
    fn = fnOrOptions;
  } else if (fnOrOptions !== undefined && maybeFn !== undefined) {
    name = nameOrFnOrOptions;
    only = fnOrOptions.only;
    ignore = fnOrOptions.ignore;
    sanitizeExit = fnOrOptions.sanitizeExit,
      sanitizeOps = fnOrOptions.sanitizeOps,
      sanitizeResources = fnOrOptions.sanitizeResources;
    permissions = fnOrOptions.permissions;
    fn = maybeFn;
  } else {
    throw new TypeError(`Invalid arguments passed to "Deno.test/it/describe"`);
  }

  return {
    name: escapeName(name),
    fn,
    only,
    ignore,
    sanitizeExit,
    sanitizeOps,
    sanitizeResources,
    permissions,
  };
}

/**
 * @param {BddArgs} args
 */
function itInner({
  name,
  fn,
  ignore,
  only,
  sanitizeExit,
  sanitizeOps,
  sanitizeResources,
  permissions,
}) {
  if (
    !ignore && BDD_CONTEXT.stack.length > 1 &&
    BDD_CONTEXT.stack.some((x) => x.ignore)
  ) {
    ignore = true;
  }

  if (cachedOrigin == undefined) {
    cachedOrigin = op_test_get_origin();
  }

  const location = core.currentUserCallSite();

  const parent = getGroupParent();

  /** @type {BddTest} */
  const testDef = {
    id: 0,
    parentId: parent.id,
    name,
    fn,
    ignore,
    only,
    location,
  };
  parent.children.push(testDef);
  BDD_CONTEXT.total++;

  op_register_test(
    parent.id,
    fn,
    escapeName(name),
    ignore,
    only,
    sanitizeOps,
    sanitizeResources,
    location.fileName,
    location.lineNumber,
    location.columnNumber,
    registerTestIdRetBufU8,
  );

  testDef.id = registerTestIdRetBuf[0];
}

/**
 * @param {string | (() => any) | BddArgs} nameOrFnOrOptions
 * @param {(() => any) | BddArgs} [fnOrOptions]
 * @param {(() => any)} [maybeFn]
 */
function it(nameOrFnOrOptions, fnOrOptions, maybeFn) {
  const args = normalizeBddArgs(nameOrFnOrOptions, fnOrOptions, maybeFn);
  if (args.only) BDD_CONTEXT.hasOnly = true;
  itInner(args);
}
/**
 * @param {string | (() => any) | BddArgs} nameOrFnOrOptions
 * @param {(() => any) | BddArgs} [fnOrOptions]
 * @param {(() => any)} [maybeFn]
 */
it.only = (nameOrFnOrOptions, fnOrOptions, maybeFn) => {
  const args = normalizeBddArgs(nameOrFnOrOptions, fnOrOptions, maybeFn);
  BDD_CONTEXT.hasOnly = true;
  args.only = true;
  args.ignore = false;
  itInner(args);
};
/**
 * @param {string | (() => any) | BddArgs} nameOrFnOrOptions
 * @param {(() => any) | BddArgs} [fnOrOptions]
 * @param {(() => any)} [maybeFn]
 */
it.ignore = (nameOrFnOrOptions, fnOrOptions, maybeFn) => {
  const args = normalizeBddArgs(nameOrFnOrOptions, fnOrOptions, maybeFn);
  args.ignore = true;
  args.only = false;
  itInner(args);
};
it.skip = it.ignore;

/** @type {(x: TestGroup | BddTest) => x is TestGroup} */
function isTestGroup(x) {
  return "beforeAll" in x;
}

/**
 * @returns {TestGroup}
 */
function getGroupParent() {
  return /** @type {TestGroup} */ (BDD_CONTEXT.stack.at(-1));
}

/**
 * @param {BddArgs} args
 */
function describeInner(
  {
    name,
    fn,
    ignore,
    only,
    sanitizeExit,
    sanitizeOps,
    sanitizeResources,
    permissions,
  },
) {
  // No-op if we're not running in `deno test` subcommand.
  if (typeof op_register_test !== "function") {
    return;
  }

  const parent = getGroupParent();
  op_test_group_register(registerTestGroupIdRetBufU8, name, parent.id);
  const id = registerTestGroupIdRetBuf[0];

  /** @type {TestGroup} */
  const group = {
    id,
    parentId: parent.id,
    name,
    ignore,
    only,
    children: [],
    beforeAll: null,
    beforeEach: null,
    afterAll: null,
    afterEach: null,
    sanitizeExit,
    sanitizeOps,
    sanitizeResources,
    permissions,
  };
  parent.children.push(group);
  BDD_CONTEXT.stack.push(group);

  try {
    fn();
  } finally {
    let allIgnore = true;
    let onlyChildCount = 0;

    for (let i = 0; i < group.children.length; i++) {
      const child = group.children[i];

      if (!child.ignore) allIgnore = false;
      if (!isTestGroup(child) && child.only) {
        onlyChildCount++;
      }
    }

    if (!group.ignore) {
      group.ignore = allIgnore;
    }

    if (!group.ignore) {
      if (onlyChildCount > 0) {
        group.only = true;

        if (onlyChildCount < group.children.length - 1) {
          for (let i = 0; i < group.children.length; i++) {
            const child = group.children[i];

            if (!isTestGroup(child) && !child.only) {
              child.ignore = true;
            }
          }
        }
      } else if (group.only) {
        for (let i = 0; i < group.children.length; i++) {
          const child = group.children[i];
          child.only = true;
        }
      }
    }

    BDD_CONTEXT.stack.pop();
  }
}

/**
 * @param {string | (() => any) | BddArgs} nameOrFnOrOptions
 * @param {(() => any) | BddArgs} [fnOrOptions]
 * @param {(() => any)} [maybeFn]
 */
function describe(nameOrFnOrOptions, fnOrOptions, maybeFn) {
  const args = normalizeBddArgs(nameOrFnOrOptions, fnOrOptions, maybeFn);
  if (args.only) BDD_CONTEXT.hasOnly = true;
  describeInner(args);
}
/**
 * @param {string | (() => any) | BddArgs} nameOrFnOrOptions
 * @param {(() => any) | BddArgs} [fnOrOptions]
 * @param {(() => any)} [maybeFn]
 */
describe.only = (nameOrFnOrOptions, fnOrOptions, maybeFn) => {
  const args = normalizeBddArgs(nameOrFnOrOptions, fnOrOptions, maybeFn);
  BDD_CONTEXT.hasOnly = true;
  args.only = true;
  args.ignore = false;
  describeInner(args);
};
/**
 * @param {string | (() => any) | BddArgs} nameOrFnOrOptions
 * @param {(() => any) | BddArgs} [fnOrOptions]
 * @param {(() => any)} [maybeFn]
 */
describe.ignore = (nameOrFnOrOptions, fnOrOptions, maybeFn) => {
  const args = normalizeBddArgs(nameOrFnOrOptions, fnOrOptions, maybeFn);
  args.only = false;
  args.ignore = true;
  describeInner(args);
};
describe.skip = describe.ignore;

/**
 * @param {() => any} fn
 */
function beforeAll(fn) {
  getGroupParent().beforeAll = fn;
}

/**
 * @param {() => any} fn
 */
function afterAll(fn) {
  getGroupParent().afterAll = fn;
}

/**
 * @param {() => any} fn
 */
function beforeEach(fn) {
  getGroupParent().beforeEach = fn;
}
/**
 * @param {() => any} fn
 */
function afterEach(fn) {
  getGroupParent().afterEach = fn;
}

globalThis.before = beforeAll;
globalThis.beforeAll = beforeAll;
globalThis.after = afterAll;
globalThis.afterAll = afterAll;
globalThis.beforeEach = beforeEach;
globalThis.afterEach = afterEach;
globalThis.it = it;
globalThis.describe = describe;

/**
 * This function is called from Rust.
 * @param {number} seed
 * @param {...any} rest
 */
async function runTests(seed, ...rest) {
  if (BDD_CONTEXT.hasOnly) {
    ROOT_TEST_GROUP.only = ROOT_TEST_GROUP.children.some((child) => child.only);
  }

  // console.log("RUN TESTS", BDD_CONTEXT.hasOnly, seed, rest, ROOT_TEST_GROUP);
  try {
    await runGroup(seed, ROOT_TEST_GROUP);
  } finally {
    //
  }
}

/**
 * @param {number} seed
 * @param {TestGroup} group
 */
async function runGroup(seed, group) {
  op_test_group_event_start(group.id);

  if (BDD_CONTEXT.hasOnly && !group.only) {
    group.ignore = true;
  }

  if (seed > 0 && !group.ignore && group.children.length > 1) {
    shuffle(group.children, seed);
  }

  // Sort tests:
  // - non-ignored tests first (might be shuffled earlier)
  // - ignored tests second
  // - groups last
  group.children.sort(sortTestItems);

  try {
    if (!group.ignore && group.beforeAll !== null) {
      await group.beforeAll();
    }

    for (let i = 0; i < group.children.length; i++) {
      const child = group.children[i];

      if (!group.ignore && group.beforeEach !== null) {
        await group.beforeEach();
      }
      if (isTestGroup(child)) {
        await runGroup(seed, child);
      } else if (child.ignore || BDD_CONTEXT.hasOnly && !child.only) {
        op_test_event_result_ignored(child.id);
      } else {
        op_test_event_start(child.id);

        const start = DateNow();
        try {
          await child.fn();
          const elapsed = DateNow() - start;
          op_test_event_result_ok(child.id, elapsed);
        } catch (err) {
          const elapsed = DateNow() - start;
          op_test_event_result_failed(child.id, elapsed);
        }
      }

      if (!group.ignore && group.afterEach !== null) {
        await group.afterEach();
      }
    }

    if (!group.ignore && group.afterAll !== null) {
      await group.afterAll();
    }
  } finally {
    op_test_group_event_end(group.id);
  }
}

/**
 * @param {TestGroup | BddTest} a
 * @param {TestGroup | BddTest} b
 */
function sortTestItems(a, b) {
  const isAGroup = isTestGroup(a);
  const isBGroup = isTestGroup(b);
  if (isAGroup && isBGroup) return 0;
  if (isAGroup && !isBGroup) return 1;
  if (!isAGroup && isBGroup) return -1;

  if (a.ignore && b.ignore) return 0;
  if (a.ignore && !b.ignore) return 1;
  if (!a.ignore && b.ignore) return -1;

  return 0;
}

/**
 * @template T
 * @param {T[]} arr
 * @param {number} seed
 */
function shuffle(arr, seed) {
  let m = arr.length;
  let t;
  let i;

  while (m) {
    i = Math.floor(randomize(seed) * m--);
    t = arr[m];
    arr[m] = arr[i];
    arr[i] = t;
  }
}

/**
 * @param {number} seed
 * @returns {number}
 */
function randomize(seed) {
  const x = Math.sin(seed++) * 10000;
  return x - Math.floor(x);
}

// No-op if we're not running in `deno test` subcommand.
if (typeof op_register_test === "function") {
  op_register_test_run_fn(runTests);
}
