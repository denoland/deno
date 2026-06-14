// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { escapeName, withPermissions } from "ext:cli/40_test_common.js";

// TODO(mmastrac): We cannot import these from "ext:core/ops" yet
const {
  op_register_test_step,
  op_register_test,
  op_register_test_hook,
  op_test_event_exit,
  op_test_event_step_result_failed,
  op_test_event_step_result_ignored,
  op_test_event_step_result_ok,
  op_test_event_step_wait,
  op_test_get_origin,
  op_test_isolate_exit,
} = core.ops;
const {
  ArrayIsArray,
  ArrayPrototypeFilter,
  ArrayPrototypePush,
  DateNow,
  Error,
  FunctionPrototypeApply,
  JSONStringify,
  Map,
  MathTrunc,
  Number,
  NumberIsFinite,
  NumberIsInteger,
  NumberIsNaN,
  MapPrototypeGet,
  MapPrototypeSet,
  SafeArrayIterator,
  SafeRegExp,
  String,
  StringPrototypeLastIndexOf,
  StringPrototypeReplace,
  StringPrototypeSlice,
  StringPrototypeSplit,
  SymbolFor,
  SymbolToStringTag,
  TypeError,
} = primordials;

const { setExitHandler } = core.loadExtScript("ext:deno_os/30_os.js");

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
 */

/** @type {Map<number, TestState | TestStepState>} */
const testStates = new Map();

/**
 * Symbol that test functions (or their wrappers) can carry to tell the test
 * runner which source location to report for the test, instead of the call
 * site of `Deno.test()` itself.
 *
 * The value must be a string in the format `"fileName:lineNumber:columnNumber"`,
 * where `fileName` may be an absolute file URL or any remote URL.  Parsing
 * works from the right so that URL schemes (which also contain `:`) are
 * handled correctly.
 *
 * This is primarily intended for test-helper libraries (e.g. `@std/testing/bdd`)
 * that call `Deno.test()` on behalf of the user: by setting this symbol on the
 * wrapped function they can report the location in *user* code rather than the
 * location inside the library itself.
 */
const TEST_LOCATION_SYMBOL = SymbolFor("Deno.test.location");

/**
 * Parse a location string of the form `"fileName:lineNumber:columnNumber"`.
 * Parsing is done from the right so that file names that contain `:` (such as
 * `file://` or `https://` URLs) are handled correctly.
 *
 * Returns `null` if the string is not a valid location.
 *
 * @param {string} str
 * @returns {{ fileName: string, lineNumber: number, columnNumber: number } | null}
 */
function parseTestLocation(str) {
  if (typeof str !== "string") return null;
  const lastColon = StringPrototypeLastIndexOf(str, ":");
  if (lastColon <= 0) return null;
  const secondLastColon = StringPrototypeLastIndexOf(str, ":", lastColon - 1);
  if (secondLastColon <= 0) return null;
  const lineNumber = parseInt(
    StringPrototypeSlice(str, secondLastColon + 1, lastColon),
  );
  const columnNumber = parseInt(StringPrototypeSlice(str, lastColon + 1));
  if (NumberIsNaN(lineNumber) || NumberIsNaN(columnNumber)) return null;
  return {
    fileName: StringPrototypeSlice(str, 0, secondLastColon),
    lineNumber,
    columnNumber,
  };
}

// Default exit handler installed at the start of every test isolate (see
// `installTestIsolateExitHandler` below). When user code calls `Deno.exit()`
// outside of any test function - at module top level, in an `unload` event,
// or from async work that escaped a test - we don't want to kill the deno
// process. Instead we record the exit code, notify the reporter, and ask V8
// to terminate the isolate so the test runner can move on to the next file.
//
// `defaultExitHandler` is also what `assertExit` restores when a per-test
// handler finishes, so that `Deno.exit()` after a test (e.g., in an unload
// listener) is still routed to the isolate-exit path.
let defaultExitHandler = null;

function installTestIsolateExitHandler() {
  defaultExitHandler = (exitCode) => {
    op_test_isolate_exit(exitCode);
    // `op_test_isolate_exit` asks V8 to terminate execution; the throw here
    // is a defense-in-depth so the current call stack is unwound even if V8
    // doesn't check the termination flag before some intermediate frame
    // catches the (uncatchable) termination exception. Either way, the test
    // runner detects the isolate-exit via `IsolateExitInfo` in `OpState`.
    throw new Error(`Deno.exit(${exitCode}) called outside of a test`);
  };
  setExitHandler(defaultExitHandler);
}

// Wrap test function in additional assertion that handles a test case trying
// to exit the process prematurely.
//
// When `sanitizeExit` is enabled (the default), any attempt to exit fails the
// current test (and a non-zero exit code set during the test fails it too),
// allowing the remaining tests to keep running.
//
// When `sanitizeExit` is disabled, the user has opted out of failing the test,
// but we still don't want a test to silently terminate the process without a
// message and - more importantly - without reliably flushing buffered output.
// Instead we abort the whole test run: the reporter prints a message, flushes
// all output, and then exits the process with the requested code.
function assertExit(fn, isTest, sanitizeExit) {
  return async function exitSanitizer(...params) {
    setExitHandler((exitCode) => {
      if (!sanitizeExit) {
        // Hand the exit off to the test runner. This never returns - the
        // process is terminated once the reporter has flushed its output.
        op_test_event_exit(exitCode);
        return;
      }
      throw new Error(
        `${
          isTest ? "Test case" : "Bench"
        } attempted to exit with exit code: ${exitCode}`,
      );
    });

    try {
      const innerResult = await fn(...new SafeArrayIterator(params));
      if (sanitizeExit) {
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
      }
      if (innerResult) {
        return innerResult;
      }
    } finally {
      // Restore the isolate-level default handler so that a subsequent
      // top-level `Deno.exit()` (e.g., in an `unload` listener) is routed
      // back into the test runner instead of falling through to `op_exit`.
      setExitHandler(defaultExitHandler);
    }
  };
}

function wrapOuter(fn, desc) {
  return async function outerWrapped() {
    const state = MapPrototypeGet(testStates, desc.id);
    // A test may be invoked more than once when `retry`/`repeats` are set.
    // Reset any state left over from a previous invocation so steps can run
    // again and stale children aren't reported as incomplete.
    state.children = [];
    state.completed = false;
    try {
      if (desc.ignore) {
        return "ignored";
      }
      return await fn(desc) ?? "ok";
    } catch (error) {
      return { failed: { jsError: core.destructureError(error) } };
    } finally {
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

const TIMEOUT_MAX = 0x7FFFFFFF;

function encodeTimeout(value) {
  if (value === undefined || value === null) return 0;
  if (
    typeof value !== "number" || NumberIsNaN(value) ||
    !NumberIsFinite(value) || !NumberIsInteger(value) || value < 0
  ) {
    throw new TypeError(
      "Test timeout must be a non-negative integer number of milliseconds",
    );
  }
  if (value === 0) return 0;
  if (value > TIMEOUT_MAX) {
    throw new TypeError(
      "Test timeout out of range (must be between 1 and 2147483647 ms)",
    );
  }
  return value;
}

// Validates the `retry`/`repeats` test options, which are non-negative integer
// counts passed to the runner as u32 values (0 means the option is unset).
function encodeCount(value, label) {
  if (value === undefined || value === null) return 0;
  if (
    typeof value !== "number" || NumberIsNaN(value) ||
    !NumberIsFinite(value) || !NumberIsInteger(value) || value < 0
  ) {
    throw new TypeError(`Test ${label} must be a non-negative integer`);
  }
  if (value > TIMEOUT_MAX) {
    throw new TypeError(
      `Test ${label} out of range (must be between 0 and 2147483647)`,
    );
  }
  return value;
}

// As long as we're using one isolate per test, we can cache the origin since it won't change
let cachedOrigin = undefined;

// Module-level sanitizer overrides set via Deno.test.sanitizer()
// These have higher precedence than CLI flags/config but lower than per-test options
let moduleSanitizeOps = undefined;
let moduleSanitizeResources = undefined;

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
    sanitizeOps: moduleSanitizeOps ??
      Deno[Deno.internal].testSanitizeOps ?? false,
    sanitizeResources: moduleSanitizeResources ??
      Deno[Deno.internal].testSanitizeResources ?? false,
    sanitizeExit: true,
    permissions: null,
    timeout: undefined,
    retry: 0,
    repeats: 0,
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

  const locationOverride = parseTestLocation(
    testDesc.fn[TEST_LOCATION_SYMBOL],
  );
  testDesc.location = locationOverride ?? core.currentUserCallSite();
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
    testDesc.sanitizeOnly ?? true,
    encodeTimeout(testDesc.timeout),
    encodeCount(testDesc.retry, "retry"),
    encodeCount(testDesc.repeats, "repeats"),
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

function registerHook(hookType, fn) {
  // No-op if we're not running in `deno test` subcommand.
  if (typeof op_register_test_hook !== "function") {
    return;
  }

  if (typeof fn !== "function") {
    throw new TypeError(`Expected a function for ${hookType} hook`);
  }

  op_register_test_hook(hookType, fn);
}

test.beforeAll = function (fn) {
  registerHook("beforeAll", fn);
};

test.beforeEach = function (fn) {
  registerHook("beforeEach", fn);
};

test.afterEach = function (fn) {
  registerHook("afterEach", fn);
};

test.afterAll = function (fn) {
  registerHook("afterAll", fn);
};

test.sanitizer = function (options) {
  if (typeof options !== "object" || options === null) {
    throw new TypeError(
      "Deno.test.sanitizer: options must be an object",
    );
  }
  if (options.ops !== undefined) {
    moduleSanitizeOps = options.ops;
  }
  if (options.resources !== undefined) {
    moduleSanitizeResources = options.resources;
  }
};

// Matches a `printf`-style token (`%s`, `%d`, `%i`, `%f`, `%j`, `%o`, `%O`,
// `%#`, `%%`) or a `$`-prefixed object path (`$foo`, `$foo.bar`) inside a
// `Deno.test.each()` name template.
const EACH_NAME_TOKEN = new SafeRegExp(
  "%[sdifjoO#%]|\\$[\\w$]+(?:\\.[\\w$]+)*",
  "g",
);

// Stringify a value for interpolation into a generated test name. Strings are
// inserted verbatim; everything else is JSON-encoded (falling back to `String`
// for values JSON can't represent, such as `bigint` or circular objects).
function eachStringify(value) {
  if (typeof value === "string") {
    return value;
  }
  try {
    const json = JSONStringify(value);
    return json === undefined ? String(value) : json;
  } catch {
    return String(value);
  }
}

// Resolve a dotted `$`-path (e.g. `foo.bar`) against an object row.
function eachResolvePath(row, path) {
  const parts = StringPrototypeSplit(path, ".");
  let current = row;
  for (const part of new SafeArrayIterator(parts)) {
    if (current === null || current === undefined) {
      return undefined;
    }
    current = current[part];
  }
  return current;
}

// Build the name for a single `Deno.test.each()` case by interpolating the
// template against the case's row and its zero-based index.
function formatEachName(template, row, index) {
  if (typeof template !== "string") {
    throw new TypeError("Deno.test.each: test name must be a string");
  }
  const isArray = ArrayIsArray(row);
  const positional = isArray ? row : [row];
  let argIndex = 0;
  return StringPrototypeReplace(template, EACH_NAME_TOKEN, (token) => {
    if (token === "%%") {
      return "%";
    }
    if (token === "%#") {
      return String(index);
    }
    if (token[0] === "$") {
      const value = eachResolvePath(row, StringPrototypeSlice(token, 1));
      return eachStringify(value);
    }
    const value = positional[argIndex++];
    switch (token) {
      case "%s":
        return String(value);
      case "%d":
      case "%i": {
        const n = Number(value);
        return NumberIsNaN(n) ? "NaN" : String(MathTrunc(n));
      }
      case "%f":
        return String(Number(value));
      case "%j":
        return eachStringify(value);
      case "%o":
      case "%O":
        return eachStringify(value);
      default:
        return token;
    }
  });
}

// Create a `Deno.test.each()` (and `.only.each`/`.ignore.each`) implementation
// bound to the given test registration `overrides`.
function createEach(overrides) {
  return function each(cases) {
    if (!ArrayIsArray(cases)) {
      throw new TypeError(
        "Deno.test.each: expected an array of test cases",
      );
    }
    return function (name, optionsOrFn, maybeFn) {
      let options;
      let fn;
      if (typeof optionsOrFn === "function") {
        fn = optionsOrFn;
      } else {
        options = optionsOrFn;
        fn = maybeFn;
      }
      if (typeof fn !== "function") {
        throw new TypeError("Deno.test.each: missing test function");
      }

      // Report all generated tests at the user's `.each(...)(...)` call site
      // rather than inside this function.
      const callSite = core.currentUserCallSite();
      const location =
        `${callSite.fileName}:${callSite.lineNumber}:${callSite.columnNumber}`;

      let index = 0;
      for (const row of new SafeArrayIterator(cases)) {
        const caseName = formatEachName(name, row, index);
        const args = ArrayIsArray(row) ? row : [row];
        const caseFn = (t) =>
          FunctionPrototypeApply(fn, undefined, [
            ...new SafeArrayIterator(args),
            t,
          ]);
        caseFn[TEST_LOCATION_SYMBOL] = location;
        if (options === undefined) {
          testInner(caseName, caseFn, undefined, overrides);
        } else {
          testInner(caseName, options, caseFn, overrides);
        }
        index++;
      }
    };
  };
}

test.each = createEach({ __proto__: null });
test.only.each = createEach({ only: true });
test.ignore.each = createEach({ ignore: true });

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
  // Always install the exit handler - its behavior depends on `sanitizeExit`.
  testFn = assertExit(testFn, true, desc.sanitizeExit);
  if (!("parent" in desc) && desc.permissions) {
    testFn = withPermissions(testFn, desc.permissions);
  }
  return wrapOuter(testFn, desc);
}

globalThis.Deno.test = test;
globalThis.Deno[globalThis.Deno.internal].installTestIsolateExitHandler =
  installTestIsolateExitHandler;
