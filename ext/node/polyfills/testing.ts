// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";

// These ops are only available when running under `deno test`.
// Must be accessed lazily via core.ops since they are registered after the snapshot.
const ops = core.ops;
const {
  ArrayPrototypeForEach,
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  DateNow,
  Error,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  NumberIsInteger,
  ObjectDefineProperty,
  ObjectGetOwnPropertyDescriptor,
  ObjectGetPrototypeOf,
  ObjectHasOwn,
  Promise,
  PromisePrototypeThen,
  ReflectApply,
  SafeArrayIterator,
  SafeMap,
  String,
  StringPrototypeCharCodeAt,
  StringPrototypeReplaceAll,
  SymbolToStringTag,
  TypeError,
  TypedArrayPrototypeGetBuffer,
  Uint8Array,
  Uint32Array,
} = primordials;

// deno-lint-ignore no-explicit-any
type CallbackFn = (...args: any[]) => unknown;

// Inline escapeName since ext:cli/40_test_common.js is not available in the node polyfill snapshot
const ESCAPE_ASCII_CHARS: [string, string][] = [
  ["\b", "\\b"],
  ["\f", "\\f"],
  ["\t", "\\t"],
  ["\n", "\\n"],
  ["\r", "\\r"],
  ["\v", "\\v"],
];
function escapeName(name: string): string {
  for (let i = 0; i < name.length; i++) {
    const ch = StringPrototypeCharCodeAt(name, i);
    if (ch <= 13 && ch >= 8) {
      for (
        const pair of new SafeArrayIterator(ESCAPE_ASCII_CHARS)
      ) {
        name = StringPrototypeReplaceAll(name, pair[0], pair[1]);
      }
      return name;
    }
  }
  return name;
}
import { notImplemented } from "ext:deno_node/_utils.ts";
import assert from "node:assert";

// Check if we're running in `deno test` subcommand.
// Must be lazy since ops are added after snapshot.
function isTestSubcommand(): boolean {
  return typeof ops.op_register_test === "function";
}

// --------------------------------------------------------------------------
// Unhandled rejection handling for node:test
// --------------------------------------------------------------------------
// In Node.js, unhandled rejections during a test cause the test to pass
// but emit a warning. In Deno, they're fatal for the entire module.
// We install a global handler that catches them during test execution,
// preventing Deno from treating them as fatal module errors.

let errorHandlersInstalled = false;

// When a callback-style test is in progress, this holds a reject function
// so that caught async errors can unblock the pending done() callback.
let pendingCallbackReject: ((err: unknown) => void) | null = null;

function installErrorHandlers() {
  if (errorHandlersInstalled) return;
  errorHandlersInstalled = true;

  // Catch unhandled promise rejections during node:test execution.
  // In Node.js these cause the test to pass (with a warning), not crash the runner.
  globalThis.addEventListener("unhandledrejection", (event) => {
    event.preventDefault();
  });

  // Catch uncaught exceptions from async callbacks (setImmediate, setTimeout).
  // In Node.js these also cause the test to pass (with a warning).
  globalThis.addEventListener("error", (event) => {
    event.preventDefault();
    // If a callback test is pending, unblock it so the test doesn't hang
    if (pendingCallbackReject !== null) {
      pendingCallbackReject(event.error ?? new Error("uncaught error"));
      pendingCallbackReject = null;
    }
  });
}

// Safe wrapper for core.destructureError that handles objects with
// throwing inspect symbols or other problematic error objects
function safeDestructureError(error: unknown) {
  try {
    return core.destructureError(error);
  } catch {
    // If destructuring the original error fails (e.g., bad inspect symbol),
    // create a plain error with whatever string representation we can get
    try {
      return core.destructureError(new Error(String(error)));
    } catch {
      return core.destructureError(new Error("test failed"));
    }
  }
}

const registerTestIdRetBuf = new Uint32Array(1);
const registerTestIdRetBufU8 = new Uint8Array(
  TypedArrayPrototypeGetBuffer(registerTestIdRetBuf),
);

let cachedOrigin: string | undefined = undefined;
function getOrigin(): string {
  if (cachedOrigin === undefined) {
    cachedOrigin = ops.op_test_get_origin();
  }
  return cachedOrigin;
}

// --------------------------------------------------------------------------
// Assert object for t.assert
// --------------------------------------------------------------------------

const methodsToCopy = [
  "deepEqual",
  "deepStrictEqual",
  "doesNotMatch",
  "doesNotReject",
  "doesNotThrow",
  "equal",
  "fail",
  "ifError",
  "match",
  "notDeepEqual",
  "notDeepStrictEqual",
  "notEqual",
  "notStrictEqual",
  "partialDeepStrictEqual",
  "rejects",
  "strictEqual",
  "throws",
  "ok",
];

let assertObject: Record<string, unknown> | undefined = undefined;
function getAssertObject(plan?: TestPlan) {
  if (plan) {
    // When a plan is set, wrap each assertion to count it
    const obj = { __proto__: null } as Record<string, unknown>;
    ArrayPrototypeForEach(methodsToCopy, (method: string) => {
      obj[method] = (...args: unknown[]) => {
        plan.count();
        return ReflectApply(
          (assert as Record<string, CallbackFn>)[method],
          assert,
          args,
        );
      };
    });
    return obj;
  }
  if (assertObject === undefined) {
    assertObject = { __proto__: null } as Record<string, unknown>;
    ArrayPrototypeForEach(methodsToCopy, (method: string) => {
      assertObject![method] = (assert as Record<string, unknown>)[method];
    });
  }
  return assertObject;
}

// --------------------------------------------------------------------------
// Test state tracking (mirrors 40_test.js pattern)
// --------------------------------------------------------------------------

interface TestState {
  children: StepDesc[];
  completed: boolean;
  failed: boolean;
  lastError?: Error;
}

interface TestDesc {
  id: number;
  name: string;
  origin: string;
  location: { fileName: string; lineNumber: number; columnNumber: number };
  ignore: boolean;
  only: boolean;
  fn: CallbackFn;
}

interface StepDesc extends TestDesc {
  parent: TestDesc | StepDesc;
  level: number;
  rootId: number;
  rootName: string;
}

const testStates = new SafeMap<number, TestState>();

// --------------------------------------------------------------------------
// TestPlan - assertion counting (t.plan)
// --------------------------------------------------------------------------

class TestPlan {
  expected: number;
  actual: number;

  constructor(count: number) {
    if (typeof count !== "number" || count < 0 || !NumberIsInteger(count)) {
      throw new TypeError("plan count must be a non-negative integer");
    }
    this.expected = count;
    this.actual = 0;
  }

  count() {
    this.actual++;
  }

  check() {
    if (this.actual !== this.expected) {
      throw new Error(
        `plan expected ${this.expected} assertions but received ${this.actual}`,
      );
    }
  }
}

// --------------------------------------------------------------------------
// NodeTestContext - the `t` object passed to test functions
// --------------------------------------------------------------------------

function noop() {}

class NodeTestContext {
  #name: string;
  #testDesc: TestDesc | StepDesc;
  #parent: NodeTestContext | undefined;
  #skipped = false;
  #todoMarked = false;
  #abortController: AbortController;
  #plan: TestPlan | null = null;
  #afterHooks: CallbackFn[] = [];
  #beforeEachHooks: CallbackFn[] = [];
  #afterEachHooks: CallbackFn[] = [];
  #assert: Record<string, unknown> | undefined;

  constructor(
    name: string,
    testDesc: TestDesc | StepDesc,
    parent: NodeTestContext | undefined,
    abortController: AbortController,
  ) {
    this.#name = name;
    this.#testDesc = testDesc;
    this.#parent = parent;
    this.#abortController = abortController;
  }

  get [SymbolToStringTag]() {
    return "TestContext";
  }

  get name(): string {
    return this.#name;
  }

  get signal(): AbortSignal {
    return this.#abortController.signal;
  }

  get assert() {
    if (this.#assert === undefined) {
      this.#assert = getAssertObject(this.#plan ?? undefined);
    }
    return this.#assert;
  }

  get mock() {
    return mock;
  }

  get fullName(): string {
    if (this.#parent) {
      return `${this.#parent.fullName} > ${this.#name}`;
    }
    return this.#name;
  }

  diagnostic(message: string) {
    // deno-lint-ignore no-console
    console.log("DIAGNOSTIC:", message);
  }

  plan(count: number) {
    if (this.#plan !== null) {
      throw new Error("cannot set plan more than once");
    }
    this.#plan = new TestPlan(count);
    // Recreate assert object with plan-counting wrappers
    this.#assert = undefined;
  }

  get _plan(): TestPlan | null {
    return this.#plan;
  }

  skip(_message?: string) {
    this.#skipped = true;
  }

  get _skipped(): boolean {
    return this.#skipped || (this.#parent?._skipped ?? false);
  }

  todo(_message?: string) {
    this.#todoMarked = true;
    this.#skipped = true;
  }

  get _todoMarked(): boolean {
    return this.#todoMarked;
  }

  runOnly() {
    // Not implemented, but don't throw - just ignore
  }

  before(fn: CallbackFn, _options?: unknown) {
    if (typeof fn !== "function") {
      throw new TypeError("before() requires a function");
    }
    let ran = false;
    ArrayPrototypePush(this.#beforeEachHooks, () => {
      if (!ran) {
        ran = true;
        return fn();
      }
    });
  }

  after(fn: CallbackFn, _options?: unknown) {
    if (typeof fn !== "function") {
      throw new TypeError("after() requires a function");
    }
    ArrayPrototypePush(this.#afterHooks, fn);
  }

  beforeEach(fn: CallbackFn, _options?: unknown) {
    if (typeof fn !== "function") {
      throw new TypeError("beforeEach() requires a function");
    }
    ArrayPrototypePush(this.#beforeEachHooks, fn);
  }

  afterEach(fn: CallbackFn, _options?: unknown) {
    if (typeof fn !== "function") {
      throw new TypeError("afterEach() requires a function");
    }
    ArrayPrototypePush(this.#afterEachHooks, fn);
  }

  async _runBeforeEachHooks() {
    for (const hook of new SafeArrayIterator(this.#beforeEachHooks)) {
      await hook();
    }
  }

  async _runAfterEachHooks() {
    for (const hook of new SafeArrayIterator(this.#afterEachHooks)) {
      await hook();
    }
  }

  async _runAfterHooks() {
    for (const hook of new SafeArrayIterator(this.#afterHooks)) {
      await hook();
    }
  }

  test(
    name: string | CallbackFn | Record<string, unknown>,
    options?: unknown,
    fn?: CallbackFn,
  ) {
    const prepared = prepareOptions(name, options, fn, {});
    return this.#runSubtest(prepared);
  }

  async #runSubtest(prepared: PreparedTest): Promise<boolean> {
    const parentDesc = this.#testDesc;
    const state = MapPrototypeGet(testStates, parentDesc.id) as TestState;
    if (state.completed) {
      throw new Error(
        "Cannot run test step after parent scope has finished execution.",
      );
    }

    const level = ObjectHasOwn(parentDesc, "level")
      ? (parentDesc as StepDesc).level + 1
      : 1;
    const rootId = ObjectHasOwn(parentDesc, "rootId")
      ? (parentDesc as StepDesc).rootId
      : parentDesc.id;
    const rootName = ObjectHasOwn(parentDesc, "rootName")
      ? (parentDesc as StepDesc).rootName
      : parentDesc.name;

    const location = core.currentUserCallSite();
    const stepName = escapeName(prepared.name);

    const stepId = ops.op_register_test_step(
      stepName,
      location.fileName,
      location.lineNumber,
      location.columnNumber,
      level,
      parentDesc.id,
      rootId,
      escapeName(rootName),
    );

    const stepDesc: StepDesc = {
      id: stepId,
      name: stepName,
      origin: parentDesc.origin,
      location,
      ignore: prepared.options.skip || prepared.options.todo || false,
      only: prepared.options.only || false,
      fn: prepared.fn,
      parent: parentDesc,
      level,
      rootId,
      rootName,
    };

    const stepState: TestState = {
      children: [],
      completed: false,
      failed: false,
    };
    MapPrototypeSet(testStates, stepId, stepState);
    ArrayPrototypePush(state.children, stepDesc);

    ops.op_test_event_step_wait(stepId);
    const earlier = DateNow();

    if (stepDesc.ignore) {
      stepState.completed = true;
      const elapsed = DateNow() - earlier;
      ops.op_test_event_step_result_ignored(stepId, elapsed);
      return true;
    }

    const childAbortController = new AbortController();
    const childContext = new NodeTestContext(
      prepared.name,
      stepDesc,
      this,
      childAbortController,
    );

    let ok = true;
    try {
      // Run parent's beforeEach hooks for this child
      await this._runBeforeEachHooks();

      // Run the test function
      if (prepared.fn.length >= 2) {
        // Callback-style
        await new Promise<void>((resolve, reject) => {
          pendingCallbackReject = reject;
          const done = (err?: Error) => {
            pendingCallbackReject = null;
            if (err) reject(err);
            else resolve();
          };
          try {
            const result = ReflectApply(prepared.fn, childContext, [
              childContext,
              done,
            ]);
            if (
              result !== null && result !== undefined &&
              typeof result.then === "function"
            ) {
              PromisePrototypeThen(result, undefined, (err: unknown) => {
                pendingCallbackReject = null;
                reject(err);
              });
            }
          } catch (err) {
            pendingCallbackReject = null;
            reject(err);
          }
        });
      } else {
        await ReflectApply(prepared.fn, childContext, [childContext]);
      }

      // Check plan
      if (childContext._plan !== null) {
        childContext._plan.check();
      }

      // Run after hooks
      await childContext._runAfterHooks();
      // Run parent's afterEach hooks
      await this._runAfterEachHooks();
    } catch (err) {
      if (!childContext._skipped) {
        ok = false;
        stepState.failed = true;
        stepState.lastError = err;
      }
      try {
        await childContext._runAfterHooks();
      } catch { /* already failing */ }
      try {
        await this._runAfterEachHooks();
      } catch { /* already failing */ }
    }

    // Report incomplete children
    for (
      const childDesc of new SafeArrayIterator(stepState.children)
    ) {
      const childState = MapPrototypeGet(
        testStates,
        childDesc.id,
      ) as TestState;
      if (!childState.completed) {
        ops.op_test_event_step_result_failed(childDesc.id, "incomplete", 0);
      }
    }
    // Check for failed children (subtests that failed within this step)
    let failedSteps = 0;
    for (
      const childDesc of new SafeArrayIterator(stepState.children)
    ) {
      const childState = MapPrototypeGet(
        testStates,
        childDesc.id,
      ) as TestState;
      if (childState?.failed) {
        failedSteps++;
      }
    }
    if (failedSteps > 0 && ok) {
      ok = false;
      stepState.failed = true;
    }

    stepState.completed = true;

    const elapsed = DateNow() - earlier;
    if (ok) {
      ops.op_test_event_step_result_ok(stepId, elapsed);
    } else if (failedSteps > 0 && !stepState.lastError) {
      ops.op_test_event_step_result_failed(
        stepId,
        { failedSteps },
        elapsed,
      );
    } else {
      ops.op_test_event_step_result_failed(
        stepId,
        {
          jsError: safeDestructureError(
            stepState.lastError ?? new Error("test failed"),
          ),
        },
        elapsed,
      );
    }

    return ok;
  }

  waitFor(
    condition: () => unknown,
    options?: { interval?: number; timeout?: number },
  ): Promise<unknown> {
    if (typeof condition !== "function") {
      throw new TypeError("condition must be a function");
    }
    const interval = options?.interval ?? 50;
    const timeout = options?.timeout ?? 1000;

    // deno-lint-ignore prefer-primordials
    const { promise, resolve, reject } = Promise.withResolvers<unknown>();
    let lastError: unknown;
    let pollerId: ReturnType<typeof setTimeout>;

    const timeoutId = setTimeout(() => {
      clearTimeout(pollerId);
      const err = new Error("waitFor() timed out");
      if (lastError !== undefined) {
        (err as Error & { cause: unknown }).cause = lastError;
      }
      reject(err);
    }, timeout);

    const poller = async () => {
      try {
        const result = await condition();
        clearTimeout(timeoutId);
        resolve(result);
      } catch (err) {
        lastError = err;
        pollerId = setTimeout(poller, interval);
      }
    };

    poller();
    return promise;
  }
}

// --------------------------------------------------------------------------
// Suite tracking
// --------------------------------------------------------------------------

let currentSuite: SuiteCollector | null = null;

interface SuiteEntry {
  type: "test" | "suite";
  prepared: PreparedTest;
}

class SuiteCollector {
  name: string;
  options: Record<string, unknown>;
  entries: SuiteEntry[] = [];
  parent: SuiteCollector | null;
  #beforeHooks: CallbackFn[] = [];
  #afterHooks: CallbackFn[] = [];
  #beforeEachHooks: CallbackFn[] = [];
  #afterEachHooks: CallbackFn[] = [];

  constructor(
    name: string,
    options: Record<string, unknown>,
    parent: SuiteCollector | null,
  ) {
    this.name = name;
    this.options = options;
    this.parent = parent;
  }

  addBefore(fn: CallbackFn) {
    ArrayPrototypePush(this.#beforeHooks, fn);
  }

  addAfter(fn: CallbackFn) {
    ArrayPrototypePush(this.#afterHooks, fn);
  }

  addBeforeEach(fn: CallbackFn) {
    ArrayPrototypePush(this.#beforeEachHooks, fn);
  }

  addAfterEach(fn: CallbackFn) {
    ArrayPrototypePush(this.#afterEachHooks, fn);
  }

  get beforeHooks() {
    return this.#beforeHooks;
  }

  get afterHooks() {
    return this.#afterHooks;
  }

  get beforeEachHooks() {
    return this.#beforeEachHooks;
  }

  get afterEachHooks() {
    return this.#afterEachHooks;
  }
}

// --------------------------------------------------------------------------
// Argument parsing helpers
// --------------------------------------------------------------------------

interface PreparedTest {
  name: string;
  fn: CallbackFn;
  options: Record<string, unknown>;
}

function prepareOptions(
  name: unknown,
  options: unknown,
  fn: unknown,
  overrides: Record<string, unknown>,
): PreparedTest {
  if (typeof name === "function") {
    fn = name;
  } else if (name !== null && typeof name === "object") {
    fn = options;
    options = name;
  } else if (typeof options === "function") {
    fn = options;
  }

  if (options === null || typeof options !== "object") {
    options = {};
  }

  const finalOptions = {
    ...(options as Record<string, unknown>),
    ...overrides,
  };

  if (typeof fn !== "function") {
    fn = noop;
  }

  if (typeof name !== "string" || name === "") {
    name = (fn as CallbackFn).name || "<anonymous>";
  }

  return { fn: fn as CallbackFn, options: finalOptions, name: name as string };
}

// --------------------------------------------------------------------------
// Test execution wrappers
// --------------------------------------------------------------------------

function wrapTestFn(
  prepared: PreparedTest,
  suiteCollector: SuiteCollector | null,
  testIdHolder: { id: number },
): CallbackFn {
  // This function is what gets registered with op_register_test.
  // It must return a TestResult: "ok" | "ignored" | { failed: ... }
  return async function nodeTestWrapper() {
    const origin = getOrigin();
    const id = testIdHolder.id;

    const testDesc: TestDesc = {
      id,
      name: prepared.name,
      origin,
      location: { fileName: "", lineNumber: 0, columnNumber: 0 },
      ignore: false,
      only: false,
      fn: prepared.fn,
    };

    const state: TestState = {
      children: [],
      completed: false,
      failed: false,
    };
    MapPrototypeSet(testStates, id, state);

    const abortController = new AbortController();
    const ctx = new NodeTestContext(
      prepared.name,
      testDesc,
      undefined,
      abortController,
    );

    // If this is a suite, run the collector function to gather entries,
    // then execute them as steps
    if (suiteCollector) {
      return await executeSuite(suiteCollector, ctx, testDesc, state);
    }

    // Regular test
    let timeout: number | undefined;
    if (typeof prepared.options.timeout === "number") {
      timeout = prepared.options.timeout;
    }

    let timeoutId: ReturnType<typeof setTimeout> | undefined;
    let timedOut = false;

    try {
      if (timeout !== undefined && timeout !== Infinity) {
        timeoutId = setTimeout(() => {
          timedOut = true;
          abortController.abort();
        }, timeout);
      }

      if (prepared.fn.length >= 2) {
        // Callback-style test
        await new Promise<void>((resolve, reject) => {
          // Allow error handler to unblock this test if an async throw occurs
          pendingCallbackReject = reject;
          const done = (err?: Error) => {
            pendingCallbackReject = null;
            if (err) reject(err);
            else resolve();
          };
          try {
            const result = ReflectApply(prepared.fn, ctx, [ctx, done]);
            // If the function returns a thenable (async fn with done callback),
            // also listen for its rejection to avoid hanging
            if (
              result !== null && result !== undefined &&
              typeof result.then === "function"
            ) {
              PromisePrototypeThen(result, undefined, (err: unknown) => {
                pendingCallbackReject = null;
                reject(err);
              });
            }
          } catch (err) {
            pendingCallbackReject = null;
            reject(err);
          }
        });
      } else {
        await ReflectApply(prepared.fn, ctx, [ctx]);
      }

      // Check plan
      if (ctx._plan !== null) {
        ctx._plan.check();
      }

      // Run after hooks
      await ctx._runAfterHooks();

      if (timedOut) {
        return {
          failed: {
            jsError: core.destructureError(
              new Error(`test timed out after ${timeout}ms`),
            ),
          },
        };
      }

      // Check for failed steps
      let failedSteps = 0;
      for (const childDesc of new SafeArrayIterator(state.children)) {
        const childState = MapPrototypeGet(
          testStates,
          childDesc.id,
        ) as TestState;
        if (!childState.completed) {
          return { failed: "incompleteSteps" };
        }
        if (childState.failed) {
          failedSteps++;
        }
      }
      state.completed = true;

      if (ctx._skipped) {
        return "ignored";
      }

      return failedSteps === 0 ? "ok" : { failed: { failedSteps } };
    } catch (error) {
      if (ctx._skipped) {
        state.completed = true;
        return "ignored";
      }
      try {
        await ctx._runAfterHooks();
      } catch { /* already failing */ }
      state.completed = true;

      if (timedOut) {
        return {
          failed: {
            jsError: core.destructureError(
              new Error(`test timed out after ${timeout}ms`),
            ),
          },
        };
      }

      return { failed: { jsError: safeDestructureError(error) } };
    } finally {
      if (timeoutId !== undefined) {
        clearTimeout(timeoutId);
      }
      state.completed = true;
    }
  };
}

async function executeSuiteCollector(
  collector: SuiteCollector,
  ctx: NodeTestContext,
) {
  // Run before hooks
  for (const hook of new SafeArrayIterator(collector.beforeHooks)) {
    await hook();
  }

  // Wire up beforeEach/afterEach from suite to context
  for (const hook of new SafeArrayIterator(collector.beforeEachHooks)) {
    ctx.beforeEach(hook);
  }
  for (const hook of new SafeArrayIterator(collector.afterEachHooks)) {
    ctx.afterEach(hook);
  }

  // Execute each entry as a step
  for (const entry of new SafeArrayIterator(collector.entries)) {
    if (entry.type === "suite") {
      const nestedCollector =
        (entry as SuiteEntry & { _collector: SuiteCollector })._collector;
      await ctx.test(
        entry.prepared.name,
        entry.prepared.options,
        async (t: NodeTestContext) => {
          await executeSuiteCollector(nestedCollector, t);
        },
      );
    } else {
      await ctx.test(
        entry.prepared.name,
        entry.prepared.options,
        entry.prepared.fn,
      );
    }
  }

  // Run after hooks
  for (const hook of new SafeArrayIterator(collector.afterHooks)) {
    await hook();
  }
}

async function executeSuite(
  collector: SuiteCollector,
  ctx: NodeTestContext,
  _desc: TestDesc,
  state: TestState,
) {
  try {
    await executeSuiteCollector(collector, ctx);

    // Check for failed steps
    let failedSteps = 0;
    for (const childDesc of new SafeArrayIterator(state.children)) {
      const childState = MapPrototypeGet(
        testStates,
        childDesc.id,
      ) as TestState;
      if (childState?.failed) {
        failedSteps++;
      }
    }

    state.completed = true;
    return failedSteps === 0 ? "ok" : { failed: { failedSteps } };
  } catch (error) {
    state.completed = true;
    return { failed: { jsError: safeDestructureError(error) } };
  }
}

// --------------------------------------------------------------------------
// Top-level registration functions
// --------------------------------------------------------------------------

function registerTest(
  prepared: PreparedTest,
  suiteCollector: SuiteCollector | null,
) {
  if (!isTestSubcommand()) return;

  // Install error handlers on first test registration to prevent
  // unhandled rejections/exceptions from being fatal for the module
  installErrorHandlers();

  const location = core.currentUserCallSite();
  const testName = escapeName(prepared.name);

  // Create an ID holder that will be populated after registration
  const testIdHolder = { id: 0 };
  const wrappedFn = wrapTestFn(prepared, suiteCollector, testIdHolder);

  ops.op_register_test(
    wrappedFn,
    testName,
    !!prepared.options.skip || !!prepared.options.todo, // ignore
    !!prepared.options.only, // only
    false, // sanitize_ops
    false, // sanitize_resources
    location.fileName,
    location.lineNumber,
    location.columnNumber,
    registerTestIdRetBufU8,
    false, // sanitize_only
  );

  // Capture the ID from the buffer after registration
  testIdHolder.id = registerTestIdRetBuf[0];
}

function registerSuite(prepared: PreparedTest) {
  if (!isTestSubcommand()) return;

  // Collect suite entries synchronously by running the suite function
  const collector = new SuiteCollector(
    prepared.name,
    prepared.options,
    currentSuite,
  );
  const prevSuite = currentSuite;
  currentSuite = collector;
  try {
    prepared.fn();
  } finally {
    currentSuite = prevSuite;
  }

  // Register as a single test that runs the suite
  registerTest(prepared, collector);
}

// --------------------------------------------------------------------------
// Public API: test()
// --------------------------------------------------------------------------

export function test(
  name?: unknown,
  options?: unknown,
  fn?: unknown,
  overrides?: Record<string, unknown>,
) {
  const prepared = prepareOptions(name, options, fn, overrides ?? {});

  if (currentSuite) {
    ArrayPrototypePush(currentSuite.entries, {
      type: "test",
      prepared,
    });
    return;
  }

  registerTest(prepared, null);
}

test.skip = function skip(name?: unknown, options?: unknown, fn?: unknown) {
  return test(name, options, fn, { skip: true });
};

test.todo = function todo(name?: unknown, options?: unknown, fn?: unknown) {
  return test(name, options, fn, { todo: true });
};

test.only = function only(name?: unknown, options?: unknown, fn?: unknown) {
  return test(name, options, fn, { only: true });
};

// --------------------------------------------------------------------------
// Public API: describe() / suite()
// --------------------------------------------------------------------------

export function suite(
  name?: unknown,
  options?: unknown,
  fn?: unknown,
  overrides?: Record<string, unknown>,
) {
  const prepared = prepareOptions(name, options, fn, overrides ?? {});

  if (currentSuite) {
    // Nested suite - just collect
    const collector = new SuiteCollector(
      prepared.name,
      prepared.options,
      currentSuite,
    );
    const prevSuite = currentSuite;
    currentSuite = collector;
    try {
      prepared.fn();
    } finally {
      currentSuite = prevSuite;
    }
    ArrayPrototypePush(prevSuite.entries, {
      type: "suite",
      // Replace fn with noop since we already collected entries
      prepared: { ...prepared, fn: noop },
    });
    // Stash the collector on the entry so executeSuite can use the already-collected entries
    const entry = prevSuite.entries[prevSuite.entries.length - 1];
    (entry as SuiteEntry & { _collector: SuiteCollector })._collector =
      collector;
    return;
  }

  registerSuite(prepared);
}

suite.skip = function skip(name?: unknown, options?: unknown, fn?: unknown) {
  return suite(name, options, fn, { skip: true });
};
suite.todo = function todo(name?: unknown, options?: unknown, fn?: unknown) {
  return suite(name, options, fn, { todo: true });
};
suite.only = function only(name?: unknown, options?: unknown, fn?: unknown) {
  return suite(name, options, fn, { only: true });
};

export function describe(
  name?: unknown,
  options?: unknown,
  fn?: unknown,
) {
  return suite(name, options, fn, {});
}

describe.skip = function skip(name?: unknown, options?: unknown, fn?: unknown) {
  return suite.skip(name, options, fn);
};
describe.todo = function todo(name?: unknown, options?: unknown, fn?: unknown) {
  return suite.todo(name, options, fn);
};
describe.only = function only(name?: unknown, options?: unknown, fn?: unknown) {
  return suite.only(name, options, fn);
};

// --------------------------------------------------------------------------
// Public API: it()
// --------------------------------------------------------------------------

export function it(
  name?: unknown,
  options?: unknown,
  fn?: unknown,
) {
  return test(name, options, fn, {});
}

it.skip = function skip(name?: unknown, options?: unknown, fn?: unknown) {
  return test.skip(name, options, fn);
};
it.todo = function todo(name?: unknown, options?: unknown, fn?: unknown) {
  return test.todo(name, options, fn);
};
it.only = function only(name?: unknown, options?: unknown, fn?: unknown) {
  return test.only(name, options, fn);
};

// --------------------------------------------------------------------------
// Public API: Module-level hooks
// --------------------------------------------------------------------------

export function before(fn: CallbackFn) {
  if (currentSuite) {
    currentSuite.addBefore(fn);
    return;
  }
  if (!isTestSubcommand()) return;
  if (typeof fn !== "function") {
    throw new TypeError("before() requires a function");
  }
  ops.op_register_test_hook("beforeAll", fn);
}

export function after(fn: CallbackFn) {
  if (currentSuite) {
    currentSuite.addAfter(fn);
    return;
  }
  if (!isTestSubcommand()) return;
  if (typeof fn !== "function") {
    throw new TypeError("after() requires a function");
  }
  ops.op_register_test_hook("afterAll", fn);
}

export function beforeEach(fn: CallbackFn) {
  if (currentSuite) {
    currentSuite.addBeforeEach(fn);
    return;
  }
  if (!isTestSubcommand()) return;
  if (typeof fn !== "function") {
    throw new TypeError("beforeEach() requires a function");
  }
  ops.op_register_test_hook("beforeEach", fn);
}

export function afterEach(fn: CallbackFn) {
  if (currentSuite) {
    currentSuite.addAfterEach(fn);
    return;
  }
  if (!isTestSubcommand()) return;
  if (typeof fn !== "function") {
    throw new TypeError("afterEach() requires a function");
  }
  ops.op_register_test_hook("afterEach", fn);
}

// --------------------------------------------------------------------------
// Public API: run()
// --------------------------------------------------------------------------

export function run() {
  notImplemented("test.run");
}

// --------------------------------------------------------------------------
// Mock implementation
// --------------------------------------------------------------------------

const activeMocks: MockFunctionContext[] = [];

interface MockCall {
  arguments: unknown[];
  error?: Error;
  result?: unknown;
  stack: Error;
  target?: unknown;
  this: unknown;
}

class MockFunctionContext {
  #calls: MockCall[] = [];
  #implementation: ((...args: unknown[]) => unknown) | undefined;
  #restore: (() => void) | undefined;
  #times: number | undefined;
  #mocks: Map<number, CallbackFn> = new SafeMap();

  constructor(
    implementation?: (...args: unknown[]) => unknown,
    restore?: () => void,
    times?: number,
  ) {
    this.#implementation = implementation;
    this.#restore = restore;
    this.#times = times;
  }

  get calls(): readonly MockCall[] {
    return this.#calls;
  }

  callCount(): number {
    return this.#calls.length;
  }

  mockImplementation(implementation: (...args: unknown[]) => unknown): void {
    if (typeof implementation !== "function") {
      throw new TypeError("implementation must be a function");
    }
    this.#implementation = implementation;
  }

  mockImplementationOnce(
    implementation: (...args: unknown[]) => unknown,
    onCall?: number,
  ): void {
    if (typeof implementation !== "function") {
      throw new TypeError("implementation must be a function");
    }
    const nextCall = this.#calls.length;
    const call = onCall ?? nextCall;
    MapPrototypeSet(this.#mocks, call, implementation);
  }

  resetCalls(): void {
    ArrayPrototypeSplice(this.#calls, 0, this.#calls.length);
  }

  restore(): void {
    if (this.#restore) {
      this.#restore();
      this.#restore = undefined;
    }
    const idx = ArrayPrototypeIndexOf(activeMocks, this);
    if (idx !== -1) {
      ArrayPrototypeSplice(activeMocks, idx, 1);
    }
  }

  _recordCall(
    thisArg: unknown,
    args: unknown[],
    result: unknown,
    error?: Error,
  ): void {
    ArrayPrototypePush(this.#calls, {
      arguments: args,
      error,
      result,
      stack: new Error(),
      this: thisArg,
    });
  }

  _shouldMock(): boolean {
    if (this.#times === undefined) return true;
    return this.#calls.length < this.#times;
  }

  _getImplementation(): ((...args: unknown[]) => unknown) | undefined {
    return this.#implementation;
  }

  _nextImpl(): ((...args: unknown[]) => unknown) | undefined {
    const nextCall = this.#calls.length;
    const onceImpl = MapPrototypeGet(this.#mocks, nextCall);
    if (onceImpl) {
      MapPrototypeDelete(this.#mocks, nextCall);
      return onceImpl as (...args: unknown[]) => unknown;
    }
    return this.#implementation;
  }
}

function createMockFunction(
  original: ((...args: unknown[]) => unknown) | undefined,
  implementation: ((...args: unknown[]) => unknown) | undefined,
  ctx: MockFunctionContext,
): (...args: unknown[]) => unknown {
  const mockFn = function (this: unknown, ...args: unknown[]): unknown {
    const oneTimeImpl = ctx._nextImpl();
    const impl = ctx._shouldMock()
      ? (oneTimeImpl ?? implementation ?? original)
      : original;

    let result: unknown;
    let error: Error | undefined;

    try {
      result = impl ? ReflectApply(impl, this, args) : undefined;
    } catch (e) {
      error = e;
      ctx._recordCall(this, args, undefined, error);
      throw e;
    }

    ctx._recordCall(this, args, result);
    return result;
  };

  ObjectDefineProperty(mockFn, "mock", {
    __proto__: null,
    value: ctx,
    writable: false,
    enumerable: false,
    configurable: false,
  });

  return mockFn;
}

export const mock = {
  fn: (
    original?: ((...args: unknown[]) => unknown) | Record<string, unknown>,
    implementation?:
      | ((...args: unknown[]) => unknown)
      | Record<string, unknown>,
    options?: { times?: number },
  ): ((...args: unknown[]) => unknown) & { mock: MockFunctionContext } => {
    // Handle overloaded signatures: fn(options), fn(original, options)
    if (original !== null && typeof original === "object") {
      options = original as { times?: number };
      original = undefined;
      implementation = undefined;
    } else if (implementation !== null && typeof implementation === "object") {
      options = implementation as { times?: number };
      implementation = original as
        | ((...args: unknown[]) => unknown)
        | undefined;
    }

    const ctx = new MockFunctionContext(
      (implementation ?? original) as
        | ((...args: unknown[]) => unknown)
        | undefined,
      undefined,
      options?.times,
    );
    ArrayPrototypePush(activeMocks, ctx);

    const mockFn = createMockFunction(
      original as ((...args: unknown[]) => unknown) | undefined,
      (implementation ?? original) as
        | ((...args: unknown[]) => unknown)
        | undefined,
      ctx,
    );
    return mockFn as ((...args: unknown[]) => unknown) & {
      mock: MockFunctionContext;
    };
  },

  getter: (
    object: object,
    methodName: string | symbol,
    implementation?: () => unknown,
    options?: { times?: number },
  ) => {
    if (implementation !== null && typeof implementation === "object") {
      options = implementation as { times?: number };
      implementation = undefined;
    }
    // deno-lint-ignore no-explicit-any
    return mock.method(object as any, methodName, implementation, {
      ...options,
      getter: true,
    });
  },

  setter: (
    object: object,
    methodName: string | symbol,
    implementation?: (value: unknown) => void,
    options?: { times?: number },
  ) => {
    if (implementation !== null && typeof implementation === "object") {
      options = implementation as { times?: number };
      implementation = undefined;
    }
    // deno-lint-ignore no-explicit-any
    return mock.method(object as any, methodName, implementation, {
      ...options,
      setter: true,
    });
  },

  method: <T extends object>(
    object: T,
    methodName: keyof T,
    implementation?:
      | ((...args: unknown[]) => unknown)
      | Record<string, unknown>,
    options?: { times?: number; getter?: boolean; setter?: boolean },
  ): ((...args: unknown[]) => unknown) & { mock: MockFunctionContext } => {
    if (
      implementation !== null && typeof implementation === "object" &&
      typeof implementation !== "function"
    ) {
      options = implementation as {
        times?: number;
        getter?: boolean;
        setter?: boolean;
      };
      implementation = undefined;
    }

    const descriptor = findPropertyDescriptor(object, methodName as string);
    if (!descriptor) {
      throw new TypeError(
        `Cannot mock property '${
          String(methodName)
        }' because it does not exist`,
      );
    }

    let original: CallbackFn | undefined;
    const isGetter = options?.getter ?? false;
    const isSetter = options?.setter ?? false;

    if (isGetter) {
      original = descriptor.get as CallbackFn | undefined;
    } else if (isSetter) {
      original = descriptor.set as CallbackFn | undefined;
    } else {
      original = descriptor.value as CallbackFn | undefined;
    }

    if (typeof original !== "function") {
      throw new TypeError(
        `Cannot mock property '${
          String(methodName)
        }' because it is not a function`,
      );
    }

    const restore = () => {
      ObjectDefineProperty(object, methodName as string, descriptor);
    };

    const impl = implementation === undefined ? original : implementation;
    const ctx = new MockFunctionContext(
      impl as (...args: unknown[]) => unknown,
      restore,
      options?.times,
    );
    ArrayPrototypePush(activeMocks, ctx);

    const mockFn = createMockFunction(
      original as (...args: unknown[]) => unknown,
      impl as (...args: unknown[]) => unknown,
      ctx,
    );

    const mockDescriptor: PropertyDescriptor = {
      configurable: descriptor.configurable,
      enumerable: descriptor.enumerable,
    };

    if (isGetter) {
      mockDescriptor.get = mockFn;
      mockDescriptor.set = descriptor.set;
    } else if (isSetter) {
      mockDescriptor.get = descriptor.get;
      mockDescriptor.set = mockFn;
    } else {
      mockDescriptor.writable = descriptor.writable;
      mockDescriptor.value = mockFn;
    }

    ObjectDefineProperty(object, methodName as string, mockDescriptor);

    return mockFn as ((...args: unknown[]) => unknown) & {
      mock: MockFunctionContext;
    };
  },

  reset: (): void => {
    ArrayPrototypeForEach(activeMocks, (ctx: MockFunctionContext) => {
      ctx.resetCalls();
    });
  },

  restoreAll: (): void => {
    while (activeMocks.length > 0) {
      const ctx = activeMocks[activeMocks.length - 1];
      ctx.restore();
    }
  },

  timers: {
    enable: () => {
      notImplemented("test.mock.timers.enable");
    },
    reset: () => {
      notImplemented("test.mock.timers.reset");
    },
    tick: () => {
      notImplemented("test.mock.timers.tick");
    },
    runAll: () => {
      notImplemented("test.mock.timers.runAll");
    },
  },
};

function findPropertyDescriptor(
  obj: object,
  name: string | symbol,
): PropertyDescriptor | undefined {
  let current = obj;
  while (current !== null && current !== undefined) {
    const desc = ObjectGetOwnPropertyDescriptor(current, name);
    if (desc) return desc;
    current = ObjectGetPrototypeOf(current);
  }
  return undefined;
}

// --------------------------------------------------------------------------
// Wire up test.* properties
// --------------------------------------------------------------------------

test.test = test;
test.it = it;
test.describe = describe;
test.suite = suite;
test.mock = mock;

export default test;
