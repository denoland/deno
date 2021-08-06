// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertThrows, unitTest } from "./test_util.ts";

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
