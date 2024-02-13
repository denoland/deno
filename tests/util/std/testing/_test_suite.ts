// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
/** The options for creating a test suite with the describe function. */
export interface DescribeDefinition<T> extends Omit<Deno.TestDefinition, "fn"> {
  fn?: () => void;
  /**
   * The `describe` function returns a `TestSuite` representing the group of tests.
   * If `describe` is called within another `describe` calls `fn`, the suite will default to that parent `describe` calls returned `TestSuite`.
   * If `describe` is not called within another `describe` calls `fn`, the suite will default to the `TestSuite` representing the global group of tests.
   */
  suite?: TestSuite<T>;
  /** Run some shared setup before all of the tests in the suite. */
  beforeAll?:
    | ((this: T) => void | Promise<void>)
    | ((this: T) => void | Promise<void>)[];
  /** Run some shared teardown after all of the tests in the suite. */
  afterAll?:
    | ((this: T) => void | Promise<void>)
    | ((this: T) => void | Promise<void>)[];
  /** Run some shared setup before each test in the suite. */
  beforeEach?:
    | ((this: T) => void | Promise<void>)
    | ((this: T) => void | Promise<void>)[];
  /** Run some shared teardown after each test in the suite. */
  afterEach?:
    | ((this: T) => void | Promise<void>)
    | ((this: T) => void | Promise<void>)[];
}

/** The options for creating an individual test case with the it function. */
export interface ItDefinition<T> extends Omit<Deno.TestDefinition, "fn"> {
  fn: (this: T, t: Deno.TestContext) => void | Promise<void>;
  /**
   * The `describe` function returns a `TestSuite` representing the group of tests.
   * If `it` is called within a `describe` calls `fn`, the suite will default to that parent `describe` calls returned `TestSuite`.
   * If `it` is not called within a `describe` calls `fn`, the suite will default to the `TestSuite` representing the global group of tests.
   */
  suite?: TestSuite<T>;
}

/** The names of all the different types of hooks. */
export type HookNames = "beforeAll" | "afterAll" | "beforeEach" | "afterEach";

/** Optional test definition keys. */
const optionalTestDefinitionKeys: (keyof Deno.TestDefinition)[] = [
  "only",
  "permissions",
  "ignore",
  "sanitizeExit",
  "sanitizeOps",
  "sanitizeResources",
];

/** Optional test step definition keys. */
const optionalTestStepDefinitionKeys: (keyof Deno.TestStepDefinition)[] = [
  "ignore",
  "sanitizeExit",
  "sanitizeOps",
  "sanitizeResources",
];

/**
 * A group of tests.
 */
export interface TestSuite<T> {
  symbol: symbol;
}

/**
 * An internal representation of a group of tests.
 */
export class TestSuiteInternal<T> implements TestSuite<T> {
  symbol: symbol;
  protected describe: DescribeDefinition<T>;
  protected steps: (TestSuiteInternal<T> | ItDefinition<T>)[];
  protected hasOnlyStep: boolean;

  constructor(describe: DescribeDefinition<T>) {
    this.describe = describe;
    this.steps = [];
    this.hasOnlyStep = false;

    const { suite } = describe;
    if (suite && !TestSuiteInternal.suites.has(suite.symbol)) {
      throw new Error("suite does not represent a registered test suite");
    }
    const testSuite = suite
      ? TestSuiteInternal.suites.get(suite.symbol)
      : TestSuiteInternal.current;
    this.symbol = Symbol();
    TestSuiteInternal.suites.set(this.symbol, this);

    const { fn } = describe;
    if (fn) {
      const temp = TestSuiteInternal.current;
      TestSuiteInternal.current = this;
      try {
        fn();
      } finally {
        TestSuiteInternal.current = temp;
      }
    }

    if (testSuite) {
      TestSuiteInternal.addStep(testSuite, this);
    } else {
      const {
        name,
        ignore,
        permissions,
        sanitizeExit,
        sanitizeOps,
        sanitizeResources,
      } = describe;
      let { only } = describe;
      if (!ignore && this.hasOnlyStep) {
        only = true;
      }
      TestSuiteInternal.registerTest({
        name,
        ignore,
        only,
        permissions,
        sanitizeExit,
        sanitizeOps,
        sanitizeResources,
        fn: async (t) => {
          TestSuiteInternal.runningCount++;
          try {
            const context = {} as T;
            const { beforeAll } = this.describe;
            if (typeof beforeAll === "function") {
              await beforeAll.call(context);
            } else if (beforeAll) {
              for (const hook of beforeAll) {
                await hook.call(context);
              }
            }
            try {
              TestSuiteInternal.active.push(this.symbol);
              await TestSuiteInternal.run(this, context, t);
            } finally {
              TestSuiteInternal.active.pop();
              const { afterAll } = this.describe;
              if (typeof afterAll === "function") {
                await afterAll.call(context);
              } else if (afterAll) {
                for (const hook of afterAll) {
                  await hook.call(context);
                }
              }
            }
          } finally {
            TestSuiteInternal.runningCount--;
          }
        },
      });
    }
  }

  /** Stores how many test suites are executing. */
  static runningCount = 0;

  /** If a test has been registered yet. Block adding global hooks if a test has been registered. */
  static started = false;

  /** A map of all test suites by symbol. */
  // deno-lint-ignore no-explicit-any
  static suites = new Map<symbol, TestSuiteInternal<any>>();

  /** The current test suite being registered. */
  // deno-lint-ignore no-explicit-any
  static current: TestSuiteInternal<any> | null = null;

  /** The stack of tests that are actively running. */
  static active: symbol[] = [];

  /** This is used internally for testing this module. */
  static reset() {
    TestSuiteInternal.runningCount = 0;
    TestSuiteInternal.started = false;
    TestSuiteInternal.current = null;
    TestSuiteInternal.active = [];
  }

  /** This is used internally to register tests. */
  static registerTest(options: Deno.TestDefinition) {
    options = { ...options };
    optionalTestDefinitionKeys.forEach((key) => {
      if (typeof options[key] === "undefined") delete options[key];
    });
    Deno.test(options);
  }

  /** Updates all steps within top level suite to have ignore set to true if only is not set to true on step. */
  static addingOnlyStep<T>(suite: TestSuiteInternal<T>) {
    if (!suite.hasOnlyStep) {
      for (let i = 0; i < suite.steps.length; i++) {
        const step = suite.steps[i]!;
        if (!(step instanceof TestSuiteInternal) && !step.only) {
          suite.steps.splice(i--, 1);
        }
      }
      suite.hasOnlyStep = true;
    }

    const parentSuite = suite.describe.suite;
    const parentTestSuite = parentSuite &&
      TestSuiteInternal.suites.get(parentSuite.symbol);
    if (parentTestSuite) {
      TestSuiteInternal.addingOnlyStep(parentTestSuite);
    }
  }

  /** This is used internally to add steps to a test suite. */
  static addStep<T>(
    suite: TestSuiteInternal<T>,
    step: TestSuiteInternal<T> | ItDefinition<T>,
  ) {
    if (!suite.hasOnlyStep) {
      if (step instanceof TestSuiteInternal) {
        if (step.hasOnlyStep || step.describe.only) {
          TestSuiteInternal.addingOnlyStep(suite);
        }
      } else {
        if (step.only) TestSuiteInternal.addingOnlyStep(suite);
      }
    }

    if (
      !(suite.hasOnlyStep && !(step instanceof TestSuiteInternal) && !step.only)
    ) {
      suite.steps.push(step);
    }
  }

  /** This is used internally to add hooks to a test suite. */
  static setHook<T>(
    suite: TestSuiteInternal<T>,
    name: HookNames,
    fn: (this: T) => void | Promise<void>,
  ) {
    if (suite.describe[name]) {
      if (typeof suite.describe[name] === "function") {
        suite.describe[name] = [
          suite.describe[name] as ((this: T) => void | Promise<void>),
        ];
      }
      (suite.describe[name] as ((this: T) => void | Promise<void>)[]).push(fn);
    } else {
      suite.describe[name] = fn;
    }
  }

  /** This is used internally to run all steps for a test suite. */
  static async run<T>(
    suite: TestSuiteInternal<T>,
    context: T,
    t: Deno.TestContext,
  ) {
    const hasOnly = suite.hasOnlyStep || suite.describe.only || false;
    for (const step of suite.steps) {
      if (
        hasOnly && step instanceof TestSuiteInternal &&
        !(step.hasOnlyStep || step.describe.only || false)
      ) {
        continue;
      }

      const {
        name,
        fn,
        ignore,
        permissions,
        sanitizeExit,
        sanitizeOps,
        sanitizeResources,
      } = step instanceof TestSuiteInternal ? step.describe : step;

      const options: Deno.TestStepDefinition = {
        name,
        ignore,
        sanitizeExit,
        sanitizeOps,
        sanitizeResources,
        fn: async (t) => {
          if (permissions) {
            throw new Error(
              "permissions option not available for nested tests",
            );
          }
          context = { ...context };
          if (step instanceof TestSuiteInternal) {
            const { beforeAll } = step.describe;
            if (typeof beforeAll === "function") {
              await beforeAll.call(context);
            } else if (beforeAll) {
              for (const hook of beforeAll) {
                await hook.call(context);
              }
            }
            try {
              TestSuiteInternal.active.push(step.symbol);
              await TestSuiteInternal.run(step, context, t);
            } finally {
              TestSuiteInternal.active.pop();
              const { afterAll } = step.describe;
              if (typeof afterAll === "function") {
                await afterAll.call(context);
              } else if (afterAll) {
                for (const hook of afterAll) {
                  await hook.call(context);
                }
              }
            }
          } else {
            await TestSuiteInternal.runTest(t, fn!, context);
          }
        },
      };
      optionalTestStepDefinitionKeys.forEach((key) => {
        if (typeof options[key] === "undefined") delete options[key];
      });
      await t.step(options);
    }
  }

  static async runTest<T>(
    t: Deno.TestContext,
    fn: (this: T, t: Deno.TestContext) => void | Promise<void>,
    context: T,
    activeIndex = 0,
  ) {
    const suite = TestSuiteInternal.active[activeIndex];
    const testSuite = suite && TestSuiteInternal.suites.get(suite);
    if (testSuite) {
      if (activeIndex === 0) context = { ...context };
      const { beforeEach } = testSuite.describe;
      if (typeof beforeEach === "function") {
        await beforeEach.call(context);
      } else if (beforeEach) {
        for (const hook of beforeEach) {
          await hook.call(context);
        }
      }
      try {
        await TestSuiteInternal.runTest(t, fn, context, activeIndex + 1);
      } finally {
        const { afterEach } = testSuite.describe;
        if (typeof afterEach === "function") {
          await afterEach.call(context);
        } else if (afterEach) {
          for (const hook of afterEach) {
            await hook.call(context);
          }
        }
      }
    } else {
      await fn.call(context, t);
    }
  }
}
