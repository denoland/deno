// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertThrows, test } from "./test_util.ts";

test(function testFnOverloading(): void {
  // just verifying that you can use this test definition syntax
  Deno.test("test fn overloading", (): void => {});
});

test(function nameOfTestCaseCantBeEmpty(): void {
  assertThrows(
    () => {
      Deno.test("", () => {});
    },
    Error,
    "The name of test case can't be empty"
  );
  assertThrows(
    () => {
      Deno.test({
        name: "",
        fn: () => {}
      });
    },
    Error,
    "The name of test case can't be empty"
  );
});

test(function testFnCantBeAnonymous(): void {
  assertThrows(
    () => {
      Deno.test(function() {});
    },
    Error,
    "Test function can't be anonymous"
  );
});
