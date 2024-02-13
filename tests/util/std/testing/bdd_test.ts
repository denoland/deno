// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertObjectMatch,
  assertStrictEquals,
} from "../assert/mod.ts";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  it,
} from "./bdd.ts";
import { TestSuiteInternal } from "./_test_suite.ts";
import { assertSpyCall, assertSpyCalls, Spy, spy, stub } from "./mock.ts";

Deno.test("global", async (t) => {
  class TestContext implements Deno.TestContext {
    name: string;
    origin: string;
    steps: TestContext[];
    spies: {
      step: Spy;
    };

    constructor(name: string) {
      this.name = name;
      this.origin = "origin";
      this.spies = {
        step: spy(this, "step"),
      };
      this.steps = [];
    }

    async step(t: Deno.TestStepDefinition): Promise<boolean>;
    async step(
      name: string,
      fn: (t: Deno.TestContext) => void | Promise<void>,
    ): Promise<boolean>;
    async step(
      fn: (t: Deno.TestContext) => void | Promise<void>,
    ): Promise<boolean>;
    async step(
      tOrNameOrFn:
        | Deno.TestStepDefinition
        | string
        | ((t: Deno.TestContext) => void | Promise<void>),
      fn?: (t: Deno.TestContext) => void | Promise<void>,
    ): Promise<boolean> {
      let ignore = false;
      if (typeof tOrNameOrFn === "function") {
        ignore = false;
        fn = tOrNameOrFn;
      } else if (typeof tOrNameOrFn === "object") {
        ignore = tOrNameOrFn.ignore ?? false;
        fn = tOrNameOrFn.fn;
      }

      const name = typeof tOrNameOrFn === "string"
        ? tOrNameOrFn
        : tOrNameOrFn.name;
      const context = new TestContext(name);
      this.steps.push(context);
      if (!ignore) {
        await fn!(context);
      }
      return !ignore;
    }
  }

  const baseStepOptions: Omit<Deno.TestStepDefinition, "name" | "fn"> = {
    ignore: false,
    sanitizeExit: true,
    sanitizeOps: true,
    sanitizeResources: true,
  };

  const baseOptions: Omit<Deno.TestDefinition, "name" | "fn"> = {
    ...baseStepOptions,
    only: false,
    permissions: "inherit",
  };

  interface GlobalContext {
    allTimer: number;
    eachTimer: number;
    x?: number;
    y?: number;
  }

  let timerIdx = 1;
  const timers = new Map<number, number>();
  function hookFns() {
    timerIdx = 1;
    timers.clear();
    return {
      beforeAllFn: spy(async function (this: GlobalContext) {
        await Promise.resolve();
        this.allTimer = timerIdx++;
        timers.set(this.allTimer, setTimeout(() => {}, 10000));
      }),
      afterAllFn: spy(async function (this: GlobalContext) {
        await Promise.resolve();
        clearTimeout(timers.get(this.allTimer));
      }),
      beforeEachFn: spy(async function (this: GlobalContext) {
        await Promise.resolve();
        this.eachTimer = timerIdx++;
        timers.set(this.eachTimer, setTimeout(() => {}, 10000));
      }),
      afterEachFn: spy(async function (this: GlobalContext) {
        await Promise.resolve();
        clearTimeout(timers.get(this.eachTimer));
      }),
    };
  }

  await t.step("global hooks", async () => {
    const test = stub(Deno, "test"),
      fns = [spy(), spy()],
      { beforeAllFn, afterAllFn, beforeEachFn, afterEachFn } = hookFns();

    const context = new TestContext("global");
    try {
      beforeAll(beforeAllFn);
      afterAll(afterAllFn);

      beforeEach(beforeEachFn);
      afterEach(afterEachFn);

      assertEquals(it({ name: "example 1", fn: fns[0] }), undefined);
      assertEquals(it({ name: "example 2", fn: fns[1] }), undefined);

      assertSpyCalls(fns[0], 0);
      assertSpyCalls(fns[1], 0);

      assertSpyCall(test, 0);
      const call = test.calls[0];
      const options = call.args[0] as Deno.TestDefinition;
      assertEquals(Object.keys(options).sort(), ["fn", "name"]);
      assertEquals(options.name, "global");

      const result = options.fn(context);
      assertStrictEquals(Promise.resolve(result), result);
      assertEquals(await result, undefined);
      assertSpyCalls(context.spies.step, 2);
    } finally {
      TestSuiteInternal.reset();
      test.restore();
    }

    let fn = fns[0];
    assertSpyCall(fn, 0, {
      self: { allTimer: 1, eachTimer: 2 },
      args: [context.steps[0]],
      returned: undefined,
    });
    assertSpyCalls(fn, 1);

    fn = fns[1];
    assertSpyCall(fn, 0, {
      self: { allTimer: 1, eachTimer: 3 },
      args: [context.steps[1]],
      returned: undefined,
    });
    assertSpyCalls(fn, 1);

    assertSpyCalls(beforeAllFn, 1);
    assertSpyCalls(afterAllFn, 1);
    assertSpyCalls(beforeEachFn, 2);
    assertSpyCalls(afterEachFn, 2);
  });

  await t.step("it", async (t) => {
    /**
     * Asserts that `Deno.test` is called with the correct options for the `it` call in the callback function.
     * This is used to reduce code duplication when testing calling `it` with different call signatures.
     */
    async function assertOptions<T>(
      expectedOptions: Omit<Deno.TestDefinition, "name" | "fn">,
      cb: (fn: Spy) => void,
    ) {
      const test = stub(Deno, "test");
      const fn = spy();
      try {
        cb(fn);

        assertSpyCalls(fn, 0);
        assertSpyCall(test, 0);
        const call = test.calls[0];
        const options = call.args[0] as Deno.TestDefinition;
        assertEquals(
          Object.keys(options).sort(),
          ["name", "fn", ...Object.keys(expectedOptions)].sort(),
        );
        assertObjectMatch(options, {
          name: "example",
          ...expectedOptions,
        });

        const context = new TestContext("example");
        const result = options.fn(context);
        assertStrictEquals(Promise.resolve(result), result);
        assertEquals(await result, undefined);
        assertSpyCalls(context.spies.step, 0);
        assertSpyCall(fn, 0, {
          self: {},
          args: [context],
          returned: undefined,
        });
      } finally {
        TestSuiteInternal.reset();
        test.restore();
      }
    }

    /**
     * Asserts that `Deno.test` is called with just the name and function for the `it` call in the callback function.
     * This is used to reduce code duplication when testing calling `it` with different call signatures.
     */
    async function assertMinimumOptions(
      cb: (fn: Spy) => void,
    ) {
      await assertOptions({}, cb);
    }

    /**
     * Asserts that `Deno.test` is called with all of the options for the `it` call in the callback function.
     * This is used to reduce code duplication when testing calling `it` with different call signatures.
     */
    async function assertAllOptions(
      cb: (fn: Spy) => void,
    ) {
      await assertOptions(baseOptions, cb);
    }

    await t.step("signature 1", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fn) => {
            assertEquals(it({ name: "example", fn }), undefined);
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fn) => {
          assertEquals(
            it({
              name: "example",
              fn,
              ...baseOptions,
            }),
            undefined,
          );
        }));
    });

    await t.step("signature 2", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fn) => {
            assertEquals(it("example", { fn }), undefined);
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fn) => {
          assertEquals(
            it("example", {
              fn,
              ...baseOptions,
            }),
            undefined,
          );
        }));
    });

    await t.step("signature 3", async () =>
      await assertMinimumOptions((fn) => {
        assertEquals(it("example", fn), undefined);
      }));

    await t.step("signature 4", async () =>
      await assertMinimumOptions((fn) => {
        assertEquals(
          it(function example(this: void, ...args) {
            fn.apply(this, args);
          }),
          undefined,
        );
      }));

    await t.step("signature 5", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fn) => {
            assertEquals(it("example", {}, fn), undefined);
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fn) => {
          assertEquals(
            it("example", {
              ...baseOptions,
            }, fn),
            undefined,
          );
        }));
    });

    await t.step("signature 6", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fn) => {
            assertEquals(it({ name: "example" }, fn), undefined);
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fn) => {
          assertEquals(
            it({
              name: "example",
              ...baseOptions,
            }, fn),
            undefined,
          );
        }));
    });

    await t.step("signature 7", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fn) => {
            assertEquals(
              it({}, function example(this: void, ...args) {
                fn.apply(this, args);
              }),
              undefined,
            );
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fn) => {
          assertEquals(
            it({
              ...baseOptions,
            }, function example(this: void, ...args) {
              fn.apply(this, args);
            }),
            undefined,
          );
        }));
    });

    await t.step("only", async (t) => {
      /**
       * Asserts that `Deno.test` is called with just the name, only, and function for the `it.only` call in the callback function.
       * This is used to reduce code duplication when testing calling `it.only` with different call signatures.
       */
      async function assertMinimumOptions(
        cb: (fn: Spy) => void,
      ) {
        await assertOptions({ only: true }, cb);
      }

      /**
       * Asserts that `Deno.test` is called with all of the options for the `it.only` call in the callback function.
       * This is used to reduce code duplication when testing calling `it.only` with different call signatures.
       */
      async function assertAllOptions(
        cb: (fn: Spy) => void,
      ) {
        await assertOptions({ ...baseOptions, only: true }, cb);
      }

      await t.step("signature 1", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(it.only({ name: "example", fn }), undefined);
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.only({
                name: "example",
                fn,
                ...baseOptions,
              }),
              undefined,
            );
          }));
      });

      await t.step("signature 2", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(it.only("example", { fn }), undefined);
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.only("example", {
                fn,
                ...baseOptions,
              }),
              undefined,
            );
          }));
      });

      await t.step(
        "signature 3",
        async () =>
          await assertMinimumOptions((fn) => {
            assertEquals(it.only("example", fn), undefined);
          }),
      );

      await t.step(
        "signature 4",
        async () =>
          await assertMinimumOptions((fn) => {
            assertEquals(
              it.only(function example(this: void, ...args) {
                fn.apply(this, args);
              }),
              undefined,
            );
          }),
      );

      await t.step("signature 5", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(it.only("example", {}, fn), undefined);
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.only("example", {
                ...baseOptions,
              }, fn),
              undefined,
            );
          }));
      });

      await t.step("signature 6", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(it.only({ name: "example" }, fn), undefined);
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.only({
                name: "example",
                ...baseOptions,
              }, fn),
              undefined,
            );
          }));
      });

      await t.step("signature 7", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(
                it.only({}, function example(this: void, ...args) {
                  fn.apply(this, args);
                }),
                undefined,
              );
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.only({
                ...baseOptions,
              }, function example(this: void, ...args) {
                fn.apply(this, args);
              }),
              undefined,
            );
          }));
      });
    });

    await t.step("ignore", async (t) => {
      /**
       * Asserts that `Deno.test` is called with just the name, ignore, and function for the `it.ignore` call in the callback function.
       * This is used to reduce code duplication when testing calling `it.ignore` with different call signatures.
       */
      async function assertMinimumOptions(
        cb: (fn: Spy) => void,
      ) {
        await assertOptions({ ignore: true }, cb);
      }

      /**
       * Asserts that `Deno.test` is called with all of the options for the `it.ignore` call in the callback function.
       * This is used to reduce code duplication when testing calling `it.ignore` with different call signatures.
       */
      async function assertAllOptions(
        cb: (fn: Spy) => void,
      ) {
        await assertOptions({ ...baseOptions, ignore: true }, cb);
      }

      await t.step("signature 1", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(it.ignore({ name: "example", fn }), undefined);
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.ignore({
                name: "example",
                fn,
                ...baseOptions,
              }),
              undefined,
            );
          }));
      });

      await t.step("signature 2", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(it.ignore("example", { fn }), undefined);
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.ignore("example", {
                fn,
                ...baseOptions,
              }),
              undefined,
            );
          }));
      });

      await t.step(
        "signature 3",
        async () =>
          await assertMinimumOptions((fn) => {
            assertEquals(it.ignore("example", fn), undefined);
          }),
      );

      await t.step(
        "signature 4",
        async () =>
          await assertMinimumOptions((fn) => {
            assertEquals(
              it.ignore(function example(this: void, ...args) {
                fn.apply(this, args);
              }),
              undefined,
            );
          }),
      );

      await t.step("signature 5", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(it.ignore("example", {}, fn), undefined);
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.ignore("example", {
                ...baseOptions,
              }, fn),
              undefined,
            );
          }));
      });

      await t.step("signature 6", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(it.ignore({ name: "example" }, fn), undefined);
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.ignore({
                name: "example",
                ...baseOptions,
              }, fn),
              undefined,
            );
          }));
      });

      await t.step("signature 7", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fn) => {
              assertEquals(
                it.ignore({}, function example(this: void, ...args) {
                  fn.apply(this, args);
                }),
                undefined,
              );
            }),
        );

        await t.step("all options", async () =>
          await assertAllOptions((fn) => {
            assertEquals(
              it.ignore({
                ...baseOptions,
              }, function example(this: void, ...args) {
                fn.apply(this, args);
              }),
              undefined,
            );
          }));
      });

      await t.step(".skip is an alias of .ignore", () => {
        assertEquals(it.ignore, it.skip);
      });
    });
  });

  await t.step("describe", async (t) => {
    /**
     * Asserts that `Deno.test` is called with the correct options for the `describe` call in the callback function.
     * In addition to that, it asserts that the individual test cases registered with `it` use the test step API correctly.
     * This is used to reduce code duplication when testing calling `describe` with different call signatures.
     */
    async function assertOptions(
      expectedOptions: Omit<Deno.TestDefinition, "name" | "fn">,
      cb: (fns: Spy[]) => void,
    ) {
      const test = stub(Deno, "test");
      const fns = [spy(), spy()];
      try {
        cb(fns);

        assertSpyCall(test, 0);
        const call = test.calls[0];
        const options = call.args[0] as Deno.TestDefinition;
        assertEquals(
          Object.keys(options).sort(),
          ["name", "fn", ...Object.keys(expectedOptions)].sort(),
        );
        assertObjectMatch(options, {
          name: "example",
          ...expectedOptions,
        });

        assertSpyCalls(fns[0], 0);
        assertSpyCalls(fns[1], 0);

        const context = new TestContext("example");
        const result = options.fn(context);
        assertStrictEquals(Promise.resolve(result), result);
        assertEquals(await result, undefined);
        assertSpyCalls(context.spies.step, 2);

        let fn = fns[0];
        assertSpyCall(fn, 0, {
          self: {},
          args: [context.steps[0]],
          returned: undefined,
        });

        fn = fns[1];
        assertSpyCall(fn, 0, {
          self: {},
          args: [context.steps[1]],
          returned: undefined,
        });
        assertSpyCalls(fn, 1);
      } finally {
        TestSuiteInternal.reset();
        test.restore();
      }
    }

    /**
     * Asserts that `Deno.test` is called with just the name and function for the `describe` call in the callback function.
     * In addition to that, it asserts that the individual test cases registered with `it` use the test step API correctly.
     * This is used to reduce code duplication when testing calling `describe` with different call signatures.
     */
    async function assertMinimumOptions(
      cb: (fns: Spy[]) => void,
    ) {
      await assertOptions({}, cb);
    }

    /**
     * Asserts that `Deno.test` is called with all of the options for the `describe` call in the callback function.
     * In addition to that, it asserts that the individual test cases registered with `it` use the test step API correctly.
     * This is used to reduce code duplication when testing calling `describe` with different call signatures.
     */
    async function assertAllOptions(
      cb: (fns: Spy[]) => void,
    ) {
      await assertOptions({ ...baseOptions }, cb);
    }

    await t.step("signature 1", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe({ name: "example" });
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "a", fn: fns[0] }), undefined);
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fns) => {
          const suite = describe({
            name: "example",
            fn: () => {
              assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            },
            ...baseOptions,
          });
          assert(suite && typeof suite.symbol === "symbol");
          assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
        }));
    });

    await t.step(
      "signature 2",
      async () =>
        await assertMinimumOptions((fns) => {
          const suite = describe("example");
          assert(suite && typeof suite.symbol === "symbol");
          assertEquals(it({ suite, name: "a", fn: fns[0] }), undefined);
          assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
        }),
    );

    await t.step("signature 3", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe("example", {});
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "a", fn: fns[0] }), undefined);
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fns) => {
          const suite = describe("example", {
            fn: () => {
              assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            },
            ...baseOptions,
          });
          assert(suite && typeof suite.symbol === "symbol");
          assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
        }));
    });

    await t.step(
      "signature 4",
      async () =>
        await assertMinimumOptions((fns) => {
          const suite = describe("example", () => {
            assertEquals(it({ name: "a", fn: fns[0] }), undefined);
          });
          assert(suite && typeof suite.symbol === "symbol");
          assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
        }),
    );

    await t.step(
      "signature 5",
      async () =>
        await assertMinimumOptions((fns) => {
          const suite = describe(function example() {
            assertEquals(it({ name: "a", fn: fns[0] }), undefined);
          });
          assert(suite && typeof suite.symbol === "symbol");
          assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
        }),
    );

    await t.step("signature 6", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe("example", {}, () => {
              assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            });
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fns) => {
          const suite = describe("example", {
            ...baseOptions,
          }, () => {
            assertEquals(it({ name: "a", fn: fns[0] }), undefined);
          });
          assert(suite && typeof suite.symbol === "symbol");
          assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
        }));
    });

    await t.step("signature 7", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe({ name: "example" }, () => {
              assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            });
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fns) => {
          const suite = describe({
            name: "example",
            ...baseOptions,
          }, () => {
            assertEquals(it({ name: "a", fn: fns[0] }), undefined);
          });
          assert(suite && typeof suite.symbol === "symbol");
          assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
        }));
    });

    await t.step("signature 8", async (t) => {
      await t.step(
        "minimum options",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe({}, function example() {
              assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            });
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step("all options", async () =>
        await assertAllOptions((fns) => {
          const suite = describe({
            ...baseOptions,
          }, function example() {
            assertEquals(it({ name: "a", fn: fns[0] }), undefined);
          });
          assert(suite && typeof suite.symbol === "symbol");
          assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
        }));
    });

    await t.step("only", async (t) => {
      /**
       * Asserts that `Deno.test` is called with just the name, only, and function for the `describe.only` call in the callback function.
       * In addition to that, it asserts that the individual test cases registered with `it` use the test step API correctly.
       * This is used to reduce code duplication when testing calling `describe.only` with different call signatures.
       */
      async function assertMinimumOptions(
        cb: (fns: Spy[]) => void,
      ) {
        await assertOptions({ only: true }, cb);
      }

      /**
       * Asserts that `Deno.test` is called with all of the options for the `describe.only` call in the callback function.
       * In addition to that, it asserts that the individual test cases registered with `it` use the test step API correctly.
       * This is used to reduce code duplication when testing calling `describe.only` with different call signatures.
       */
      async function assertAllOptions(
        cb: (fns: Spy[]) => void,
      ) {
        await assertOptions({ ...baseOptions, only: true }, cb);
      }

      await t.step("signature 1", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.only({ name: "example" });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "a", fn: fns[0] }), undefined);
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.only({
                name: "example",
                fn: () => {
                  assertEquals(it({ name: "a", fn: fns[0] }), undefined);
                },
                ...baseOptions,
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });

      await t.step(
        "signature 2",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe.only("example");
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "a", fn: fns[0] }), undefined);
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step("signature 3", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.only("example", {});
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "a", fn: fns[0] }), undefined);
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.only("example", {
                fn: () => {
                  assertEquals(it({ name: "a", fn: fns[0] }), undefined);
                },
                ...baseOptions,
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });

      await t.step(
        "signature 4",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe.only("example", () => {
              assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            });
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step(
        "signature 5",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe.only(function example() {
              assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            });
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step("signature 6", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.only("example", {}, () => {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.only("example", {
                ...baseOptions,
              }, () => {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });

      await t.step("signature 7", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.only({ name: "example" }, () => {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.only({
                name: "example",
                ...baseOptions,
              }, () => {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });

      await t.step("signature 8", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.only({}, function example() {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.only({
                ...baseOptions,
              }, function example() {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });
    });

    await t.step("ignore", async (t) => {
      /**
       * Asserts that `Deno.test` is called with just the name, ignore, and function for the `describe.ignore` call in the callback function.
       * In addition to that, it asserts that the individual test cases registered with `it` use the test step API correctly.
       * This is used to reduce code duplication when testing calling `describe.ignore` with different call signatures.
       */
      async function assertMinimumOptions(
        cb: (fns: Spy[]) => void,
      ) {
        await assertOptions({ ignore: true }, cb);
      }

      /**
       * Asserts that `Deno.test` is called with all of the options for the `describe.ignore` call in the callback function.
       * In addition to that, it asserts that the individual test cases registered with `it` use the test step API correctly.
       * This is used to reduce code duplication when testing calling `describe.ignore` with different call signatures.
       */
      async function assertAllOptions(
        cb: (fns: Spy[]) => void,
      ) {
        await assertOptions({ ...baseOptions, ignore: true }, cb);
      }

      await t.step("signature 1", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.ignore({ name: "example" });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "a", fn: fns[0] }), undefined);
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.ignore({
                name: "example",
                fn: () => {
                  assertEquals(it({ name: "a", fn: fns[0] }), undefined);
                },
                ...baseOptions,
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });

      await t.step(
        "signature 2",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe.ignore("example");
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "a", fn: fns[0] }), undefined);
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step("signature 3", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.ignore("example", {});
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "a", fn: fns[0] }), undefined);
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.ignore("example", {
                fn: () => {
                  assertEquals(it({ name: "a", fn: fns[0] }), undefined);
                },
                ...baseOptions,
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });

      await t.step(
        "signature 4",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe.ignore("example", () => {
              assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            });
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step(
        "signature 5",
        async () =>
          await assertMinimumOptions((fns) => {
            const suite = describe.ignore(function example() {
              assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            });
            assert(suite && typeof suite.symbol === "symbol");
            assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
          }),
      );

      await t.step("signature 6", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.ignore("example", {}, () => {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.ignore("example", {
                ...baseOptions,
              }, () => {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });

      await t.step("signature 7", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.ignore({ name: "example" }, () => {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.ignore({
                name: "example",
                ...baseOptions,
              }, () => {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });

      await t.step("signature 8", async (t) => {
        await t.step(
          "minimum options",
          async () =>
            await assertMinimumOptions((fns) => {
              const suite = describe.ignore({}, function example() {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );

        await t.step(
          "all options",
          async () =>
            await assertAllOptions((fns) => {
              const suite = describe.ignore({
                ...baseOptions,
              }, function example() {
                assertEquals(it({ name: "a", fn: fns[0] }), undefined);
              });
              assert(suite && typeof suite.symbol === "symbol");
              assertEquals(it({ suite, name: "b", fn: fns[1] }), undefined);
            }),
        );
      });

      await t.step(".skip is an alias of .ignore", () => {
        assertEquals(describe.ignore, describe.skip);
      });
    });

    await t.step("nested only", async (t) => {
      /**
       * Asserts that when only is used on a nested `describe` or `it` call, it will be the only test case or suite that runs in the file.
       * This is used to reduce code duplication when testing calling `describe.ignore` with different call signatures.
       */
      async function assertOnly(
        cb: (fns: Spy[]) => void,
      ) {
        const test = stub(Deno, "test");
        const fns = [spy(), spy(), spy()];
        try {
          cb(fns);

          assertSpyCall(test, 0);
          const call = test.calls[0];
          const options = call.args[0] as Deno.TestDefinition;
          assertEquals(
            Object.keys(options).sort(),
            ["name", "only", "fn"].sort(),
          );
          assertObjectMatch(options, {
            name: "example",
            only: true,
          });

          assertSpyCalls(fns[0], 0);
          assertSpyCalls(fns[1], 0);

          const context = new TestContext("example");
          const result = options.fn(context);
          assertStrictEquals(Promise.resolve(result), result);
          assertEquals(await result, undefined);
          assertSpyCalls(context.spies.step, 1);

          let fn = fns[0];
          assertSpyCalls(fn, 0);

          fn = fns[1];
          assertSpyCall(fn, 0, {
            self: {},
            returned: undefined,
          });
          assertSpyCalls(fn, 1);

          fn = fns[2];
          assertSpyCalls(fn, 0);
        } finally {
          TestSuiteInternal.reset();
          test.restore();
        }
      }

      await t.step("it", async () =>
        await assertOnly((fns) => {
          describe("example", () => {
            assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            assertEquals(it.only({ name: "b", fn: fns[1] }), undefined);
            assertEquals(it({ name: "c", fn: fns[2] }), undefined);
          });
        }));

      await t.step("nested it", async () =>
        await assertOnly((fns) => {
          describe("example", () => {
            assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            describe("nested", () => {
              assertEquals(it.only({ name: "b", fn: fns[1] }), undefined);
            });
            assertEquals(it({ name: "c", fn: fns[2] }), undefined);
          });
        }));

      await t.step("describe", async () =>
        await assertOnly((fns) => {
          describe("example", () => {
            assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            describe.only("nested", () => {
              assertEquals(it({ name: "b", fn: fns[1] }), undefined);
            });
            assertEquals(it({ name: "c", fn: fns[2] }), undefined);
          });
        }));

      await t.step("nested describe", async () =>
        await assertOnly((fns) => {
          describe("example", () => {
            assertEquals(it({ name: "a", fn: fns[0] }), undefined);
            describe("nested", () => {
              describe.only("nested 2", () => {
                assertEquals(it({ name: "b", fn: fns[1] }), undefined);
              });
            });
            assertEquals(it({ name: "c", fn: fns[2] }), undefined);
          });
        }));
    });

    await t.step("flat child only", async (t) => {
      /**
       * Asserts that when only is used on a child `describe` or `it` call, it will be the only test case or suite that runs within the top test suite.
       * This demonstrates the issue where `Deno.test` is called without `only` even though one of it's child steps are focused.
       * This is used to reduce code duplication when testing calling `describe.ignore` with different call signatures.
       */
      async function assertOnly(
        cb: (fns: Spy[]) => void,
      ) {
        const test = stub(Deno, "test");
        const fns = [spy(), spy(), spy()];
        try {
          cb(fns);

          assertSpyCall(test, 0);
          const call = test.calls[0];
          const options = call.args[0] as Deno.TestDefinition;
          assertEquals(
            Object.keys(options).sort(),
            ["name", "fn"].sort(),
          );
          assertObjectMatch(options, {
            name: "example",
          });

          assertSpyCalls(fns[0], 0);
          assertSpyCalls(fns[1], 0);

          const context = new TestContext("example");
          const result = options.fn(context);
          assertStrictEquals(Promise.resolve(result), result);
          assertEquals(await result, undefined);
          assertSpyCalls(context.spies.step, 1);

          let fn = fns[0];
          assertSpyCalls(fn, 0);

          fn = fns[1];
          assertSpyCall(fn, 0, {
            self: {},
            returned: undefined,
          });
          assertSpyCalls(fn, 1);

          fn = fns[2];
          assertSpyCalls(fn, 0);
        } finally {
          TestSuiteInternal.reset();
          test.restore();
        }
      }

      await t.step("it", async () =>
        await assertOnly((fns) => {
          const suite = describe("example");
          assertEquals(it({ name: "a", suite, fn: fns[0] }), undefined);
          assertEquals(it.only({ name: "b", suite, fn: fns[1] }), undefined);
          assertEquals(it({ name: "c", suite, fn: fns[2] }), undefined);
        }));

      await t.step("deep child it", async () =>
        await assertOnly((fns) => {
          const suite = describe("example");
          assertEquals(it({ name: "a", suite, fn: fns[0] }), undefined);
          const childSuite = describe(suite, "child");
          assertEquals(
            it.only({ name: "b", suite: childSuite, fn: fns[1] }),
            undefined,
          );
          assertEquals(it({ name: "c", suite, fn: fns[2] }), undefined);
        }));

      await t.step("describe", async () =>
        await assertOnly((fns) => {
          const suite = describe("example");
          assertEquals(it({ name: "a", suite, fn: fns[0] }), undefined);
          const childSuite = describe.only(suite, "child");
          assertEquals(
            it({ name: "b", suite: childSuite, fn: fns[1] }),
            undefined,
          );
          assertEquals(it({ name: "c", suite, fn: fns[2] }), undefined);
        }));

      await t.step(
        "deep child describe",
        async () =>
          await assertOnly((fns) => {
            const suite = describe("example");
            assertEquals(it({ name: "a", suite, fn: fns[0] }), undefined);
            const childSuite = describe(suite, "child");
            const child2Suite = describe.only(childSuite, "child 2");
            assertEquals(
              it({ name: "b", suite: child2Suite, fn: fns[1] }),
              undefined,
            );
            assertEquals(it({ name: "c", suite, fn: fns[2] }), undefined);
          }),
      );
    });

    await t.step("with hooks", async (t) => {
      /**
       * Asserts that all the different hook types are called in the correct order when the tests run.
       * This is used to reduce code duplication when testing calling `describe` with different call signatures.
       */
      async function assertHooks(
        cb: (
          options: {
            beforeAllFn: Spy;
            afterAllFn: Spy;
            beforeEachFn: Spy;
            afterEachFn: Spy;
            fns: Spy[];
          },
        ) => void,
      ) {
        const test = stub(Deno, "test"),
          fns = [spy(), spy()],
          { beforeAllFn, afterAllFn, beforeEachFn, afterEachFn } = hookFns();

        const context = new TestContext("example");
        try {
          cb({ beforeAllFn, afterAllFn, beforeEachFn, afterEachFn, fns });

          assertSpyCalls(fns[0], 0);
          assertSpyCalls(fns[1], 0);

          assertSpyCall(test, 0);
          const call = test.calls[0];
          const options = call.args[0] as Deno.TestDefinition;
          assertEquals(Object.keys(options).sort(), ["fn", "name"]);
          assertEquals(options.name, "example");

          const result = options.fn(context);
          assertStrictEquals(Promise.resolve(result), result);
          assertEquals(await result, undefined);
          assertSpyCalls(context.spies.step, 2);
        } finally {
          TestSuiteInternal.reset();
          test.restore();
        }

        let fn = fns[0];
        assertSpyCall(fn, 0, {
          self: { allTimer: 1, eachTimer: 2 },
          args: [context.steps[0]],
          returned: undefined,
        });
        assertSpyCalls(fn, 1);

        fn = fns[1];
        assertSpyCall(fn, 0, {
          self: { allTimer: 1, eachTimer: 3 },
          args: [context.steps[1]],
          returned: undefined,
        });
        assertSpyCalls(fn, 1);

        assertSpyCalls(beforeAllFn, 1);
        assertSpyCalls(afterAllFn, 1);
        assertSpyCalls(beforeEachFn, 2);
        assertSpyCalls(afterEachFn, 2);
      }

      await t.step(
        "in callback",
        async () =>
          await assertHooks(
            ({ beforeAllFn, afterAllFn, beforeEachFn, afterEachFn, fns }) => {
              describe("example", () => {
                beforeAll(beforeAllFn);
                afterAll(afterAllFn);

                beforeEach(beforeEachFn);
                afterEach(afterEachFn);

                assertEquals(it({ name: "example 1", fn: fns[0] }), undefined);
                assertEquals(it({ name: "example 2", fn: fns[1] }), undefined);
              });
            },
          ),
      );

      await t.step(
        "in options",
        async () =>
          await assertHooks(
            ({ beforeAllFn, afterAllFn, beforeEachFn, afterEachFn, fns }) => {
              describe({
                name: "example",
                beforeAll: beforeAllFn,
                afterAll: afterAllFn,
                beforeEach: beforeEachFn,
                afterEach: afterEachFn,
                fn: () => {
                  assertEquals(
                    it({ name: "example 1", fn: fns[0] }),
                    undefined,
                  );
                  assertEquals(
                    it({ name: "example 2", fn: fns[1] }),
                    undefined,
                  );
                },
              });
            },
          ),
      );

      await t.step(
        "nested",
        async () => {
          const test = stub(Deno, "test"),
            fns = [spy(), spy()],
            { beforeAllFn, afterAllFn, beforeEachFn, afterEachFn } = hookFns();

          const context = new TestContext("example");
          try {
            describe("example", () => {
              beforeAll(beforeAllFn);
              afterAll(afterAllFn);

              beforeEach(beforeEachFn);
              afterEach(afterEachFn);

              describe("nested", () => {
                assertEquals(it({ name: "example 1", fn: fns[0] }), undefined);
                assertEquals(it({ name: "example 2", fn: fns[1] }), undefined);
              });
            });

            assertSpyCalls(fns[0], 0);
            assertSpyCalls(fns[1], 0);

            assertSpyCall(test, 0);
            const call = test.calls[0];
            const options = call.args[0] as Deno.TestDefinition;
            assertEquals(Object.keys(options).sort(), ["fn", "name"]);
            assertEquals(options.name, "example");

            const result = options.fn(context);
            assertStrictEquals(Promise.resolve(result), result);
            assertEquals(await result, undefined);
            assertSpyCalls(context.spies.step, 1);

            assertStrictEquals(Promise.resolve(result), result);
            assertEquals(await result, undefined);
            assertSpyCalls(context.steps[0].spies.step, 2);
          } finally {
            TestSuiteInternal.reset();
            test.restore();
          }

          let fn = fns[0];
          assertSpyCall(fn, 0, {
            self: { allTimer: 1, eachTimer: 2 },
            args: [context.steps[0].steps[0]],
            returned: undefined,
          });
          assertSpyCalls(fn, 1);

          fn = fns[1];
          assertSpyCall(fn, 0, {
            self: { allTimer: 1, eachTimer: 3 },
            args: [context.steps[0].steps[1]],
            returned: undefined,
          });
          assertSpyCalls(fn, 1);

          assertSpyCalls(beforeAllFn, 1);
          assertSpyCalls(afterAllFn, 1);
          assertSpyCalls(beforeEachFn, 2);
          assertSpyCalls(afterEachFn, 2);
        },
      );

      interface NestedContext extends GlobalContext {
        allTimerNested: number;
        eachTimerNested: number;
        x: number;
        y: number;
      }

      await t.step(
        "nested with hooks",
        async () => {
          const test = stub(Deno, "test"),
            fns = [
              spy(function (this: NestedContext) {
                this.x = 2;
              }),
              spy(function (this: NestedContext) {
                this.y = 3;
              }),
            ],
            { beforeAllFn, afterAllFn, beforeEachFn, afterEachFn } = hookFns(),
            beforeAllFnNested = spy(async function (this: NestedContext) {
              await Promise.resolve();
              this.x = 1;
              this.allTimerNested = timerIdx++;
              timers.set(
                this.allTimerNested,
                setTimeout(() => {}, 10000),
              );
            }),
            afterAllFnNested = spy(
              async function (this: NestedContext) {
                await Promise.resolve();
                clearTimeout(timers.get(this.allTimerNested));
              },
            ),
            beforeEachFnNested = spy(async function (this: NestedContext) {
              await Promise.resolve();
              this.y = 2;
              this.eachTimerNested = timerIdx++;
              timers.set(
                this.eachTimerNested,
                setTimeout(() => {}, 10000),
              );
            }),
            afterEachFnNested = spy(
              async function (this: NestedContext) {
                await Promise.resolve();
                clearTimeout(timers.get(this.eachTimerNested));
              },
            );

          const context = new TestContext("example");
          try {
            describe("example", () => {
              beforeAll(beforeAllFn);
              afterAll(afterAllFn);

              beforeEach(beforeEachFn);
              afterEach(afterEachFn);

              describe("nested", () => {
                beforeAll(beforeAllFnNested);
                afterAll(afterAllFnNested);

                beforeEach(beforeEachFnNested);
                afterEach(afterEachFnNested);

                assertEquals(it({ name: "example 1", fn: fns[0] }), undefined);
                assertEquals(it({ name: "example 2", fn: fns[1] }), undefined);
              });
            });

            assertSpyCalls(fns[0], 0);
            assertSpyCalls(fns[1], 0);

            assertSpyCall(test, 0);
            const call = test.calls[0];
            const options = call.args[0] as Deno.TestDefinition;
            assertEquals(Object.keys(options).sort(), ["fn", "name"]);
            assertEquals(options.name, "example");

            const result = options.fn(context);
            assertStrictEquals(Promise.resolve(result), result);
            assertEquals(await result, undefined);
            assertSpyCalls(context.spies.step, 1);

            assertStrictEquals(Promise.resolve(result), result);
            assertEquals(await result, undefined);
            assertSpyCalls(context.steps[0].spies.step, 2);
          } finally {
            TestSuiteInternal.reset();
            test.restore();
          }

          let fn = fns[0];
          assertSpyCall(fn, 0, {
            self: {
              allTimer: 1,
              allTimerNested: 2,
              eachTimer: 3,
              eachTimerNested: 4,
              x: 2,
              y: 2,
            },
            args: [context.steps[0].steps[0]],
            returned: undefined,
          });
          assertSpyCalls(fn, 1);

          fn = fns[1];
          assertSpyCall(fn, 0, {
            self: {
              allTimer: 1,
              allTimerNested: 2,
              eachTimer: 5,
              eachTimerNested: 6,
              x: 1,
              y: 3,
            },
            args: [context.steps[0].steps[1]],
            returned: undefined,
          });
          assertSpyCalls(fn, 1);

          assertSpyCalls(beforeAllFn, 1);

          assertSpyCall(afterAllFn, 0, {
            self: {
              allTimer: 1,
            } as GlobalContext,
          });
          assertSpyCalls(afterAllFn, 1);

          assertSpyCalls(beforeEachFn, 2);

          assertSpyCall(afterEachFn, 0, {
            self: {
              allTimer: 1,
              allTimerNested: 2,
              eachTimer: 3,
              eachTimerNested: 4,
              x: 2,
              y: 2,
            } as NestedContext,
          });
          assertSpyCall(afterEachFn, 1, {
            self: {
              allTimer: 1,
              allTimerNested: 2,
              eachTimer: 5,
              eachTimerNested: 6,
              x: 1,
              y: 3,
            } as NestedContext,
          });
          assertSpyCalls(afterEachFn, 2);

          assertSpyCalls(beforeAllFnNested, 1);

          assertSpyCall(afterAllFnNested, 0, {
            self: {
              allTimer: 1,
              allTimerNested: 2,
              x: 1,
            } as NestedContext,
          });
          assertSpyCalls(afterAllFnNested, 1);

          assertSpyCalls(beforeEachFnNested, 2);

          assertSpyCall(afterEachFnNested, 0, {
            self: {
              allTimer: 1,
              allTimerNested: 2,
              eachTimer: 3,
              eachTimerNested: 4,
              x: 2,
              y: 2,
            },
          });
          assertSpyCall(afterEachFnNested, 1, {
            self: {
              allTimer: 1,
              allTimerNested: 2,
              eachTimer: 5,
              eachTimerNested: 6,
              x: 1,
              y: 3,
            },
          });
          assertSpyCalls(afterEachFnNested, 2);
        },
      );
    });
  });
});
