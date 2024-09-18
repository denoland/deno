// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects, assertThrows } from "./test_util.ts";

Deno.test(function testWrongOverloads() {
  assertThrows(
    () => {
      // @ts-ignore Testing invalid overloads
      Deno.test("some name", { fn: () => {} }, () => {});
    },
    TypeError,
    "Unexpected 'fn' field in options, test function is already provided as the third argument",
  );
  assertThrows(
    () => {
      // @ts-ignore Testing invalid overloads
      Deno.test("some name", { name: "some name2" }, () => {});
    },
    TypeError,
    "Unexpected 'name' field in options, test name is already provided as the first argument",
  );
  assertThrows(
    () => {
      // @ts-ignore Testing invalid overloads
      Deno.test(() => {});
    },
    TypeError,
    "The test function must have a name",
  );
  assertThrows(
    () => {
      // @ts-ignore Testing invalid overloads
      Deno.test(function foo() {}, {});
    },
    TypeError,
    "Unexpected second argument to Deno.test()",
  );
  assertThrows(
    () => {
      // @ts-ignore Testing invalid overloads
      Deno.test({ fn: () => {} }, function foo() {});
    },
    TypeError,
    "Unexpected 'fn' field in options, test function is already provided as the second argument",
  );
  assertThrows(
    () => {
      // @ts-ignore Testing invalid overloads
      Deno.test({});
    },
    TypeError,
    "Expected 'fn' field in the first argument to be a test function",
  );
  assertThrows(
    () => {
      // @ts-ignore Testing invalid overloads
      Deno.test({ fn: "boo!" });
    },
    TypeError,
    "Expected 'fn' field in the first argument to be a test function",
  );
});

Deno.test(function nameOfTestCaseCantBeEmpty() {
  assertThrows(
    () => {
      Deno.test("", () => {});
    },
    TypeError,
    "The test name can't be empty",
  );
  assertThrows(
    () => {
      Deno.test({
        name: "",
        fn: () => {},
      });
    },
    TypeError,
    "The test name can't be empty",
  );
});

Deno.test(async function invalidStepArguments(t) {
  await assertRejects(
    async () => {
      // deno-lint-ignore no-explicit-any
      await (t as any).step("test");
    },
    TypeError,
    "Expected function for second argument",
  );

  await assertRejects(
    async () => {
      // deno-lint-ignore no-explicit-any
      await (t as any).step("test", "not a function");
    },
    TypeError,
    "Expected function for second argument",
  );

  await assertRejects(
    async () => {
      // deno-lint-ignore no-explicit-any
      await (t as any).step();
    },
    TypeError,
    "Expected a test definition or name and function",
  );

  await assertRejects(
    async () => {
      // deno-lint-ignore no-explicit-any
      await (t as any).step(() => {});
    },
    TypeError,
    "The step function must have a name",
  );
});

Deno.test(async function nameOnTextContext(t1) {
  await assertEquals(t1.name, "nameOnTextContext");
  await t1.step("step", async (t2) => {
    await assertEquals(t2.name, "step");
    await t2.step("nested step", async (t3) => {
      await assertEquals(t3.name, "nested step");
    });
  });
});

Deno.test(async function originOnTextContext(t1) {
  await assertEquals(t1.origin, Deno.mainModule);
  await t1.step("step", async (t2) => {
    await assertEquals(t2.origin, Deno.mainModule);
    await t2.step("nested step", async (t3) => {
      await assertEquals(t3.origin, Deno.mainModule);
    });
  });
});

Deno.test(async function parentOnTextContext(t1) {
  await assertEquals(t1.parent, undefined);
  await t1.step("step", async (t2) => {
    await assertEquals(t1, t2.parent);
    await t2.step("nested step", async (t3) => {
      await assertEquals(t2, t3.parent);
    });
  });
});

Deno.test("explicit undefined for boolean options", {
  ignore: undefined,
  only: undefined,
}, () => {});
