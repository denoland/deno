// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertRejects, assertThrows, unitTest } from "./test_util.ts";

unitTest(function testFnOverloading() {
  // just verifying that you can use this test definition syntax
  Deno.test("test fn overloading", () => {});
});

unitTest(function nameOfTestCaseCantBeEmpty() {
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

unitTest(function invalidStepArguments(t) {
  assertRejects(
    async () => {
      // deno-lint-ignore no-explicit-any
      await (t as any).step("test");
    },
    TypeError,
    "Expected function for second argument.",
  );

  assertRejects(
    async () => {
      // deno-lint-ignore no-explicit-any
      await (t as any).step("test", "not a function");
    },
    TypeError,
    "Expected function for second argument.",
  );

  assertRejects(
    async () => {
      // deno-lint-ignore no-explicit-any
      await (t as any).step();
    },
    TypeError,
    "Expected a test definition or name and function.",
  );

  assertRejects(
    async () => {
      // deno-lint-ignore no-explicit-any
      await (t as any).step(() => {});
    },
    TypeError,
    "Expected a test definition or name and function.",
  );
});
