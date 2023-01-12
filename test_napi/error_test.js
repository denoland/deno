// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertThrows,
  loadTestLibrary,
} from "./common.js";

const test_error = loadTestLibrary();

const theError = new Error("Some error");
const theTypeError = new TypeError("Some type error");
const theSyntaxError = new SyntaxError("Some syntax error");
const theRangeError = new RangeError("Some type error");
const theReferenceError = new ReferenceError("Some reference error");
const theURIError = new URIError("Some URI error");
const theEvalError = new EvalError("Some eval error");

Deno.test("napi error", function () {
  class MyError extends Error {}
  const myError = new MyError("Some MyError");

  // Test that native error object is correctly classed
  assertEquals(test_error.checkError(theError), true);

  // Test that native type error object is correctly classed
  assertEquals(test_error.checkError(theTypeError), true);

  // Test that native syntax error object is correctly classed
  assertEquals(test_error.checkError(theSyntaxError), true);

  // Test that native range error object is correctly classed
  assertEquals(test_error.checkError(theRangeError), true);

  // Test that native reference error object is correctly classed
  assertEquals(test_error.checkError(theReferenceError), true);

  // Test that native URI error object is correctly classed
  assertEquals(test_error.checkError(theURIError), true);

  // Test that native eval error object is correctly classed
  assertEquals(test_error.checkError(theEvalError), true);

  // Test that class derived from native error is correctly classed
  assertEquals(test_error.checkError(myError), true);

  // Test that non-error object is correctly classed
  assertEquals(test_error.checkError({}), false);

  // Test that non-error primitive is correctly classed
  assertEquals(test_error.checkError("non-object"), false);

  assertThrows(
    () => {
      test_error.throwExistingError();
    },
    Error,
    "Error: existing error",
  );

  assertThrows(
    () => {
      test_error.throwError();
    },
    Error,
    "Error: error",
  );

  assertThrows(
    () => {
      test_error.throwRangeError();
    },
    RangeError,
    "RangeError: range error",
  );

  assertThrows(
    () => {
      test_error.throwTypeError();
    },
    TypeError,
    "TypeError: type error",
  );

  // assertThrows(() => {
  //   test_error.throwSyntaxError();
  // }, "SyntaxError: syntax error");

  [42, {}, [], Symbol("xyzzy"), true, "ball", undefined, null, NaN]
    .forEach((value) =>
      assertThrows(
        () => test_error.throwArbitrary(value),
        value,
      )
    );

  assertThrows(
    () => test_error.throwErrorCode(),
    {
      code: "ERR_TEST_CODE",
      message: "Error [error]",
    },
  );

  assertThrows(
    () => test_error.throwRangeErrorCode(),
    {
      code: "ERR_TEST_CODE",
      message: "RangeError [range error]",
    },
  );

  assertThrows(
    () => test_error.throwTypeErrorCode(),
    {
      code: "ERR_TEST_CODE",
      message: "TypeError [type error]",
    },
  );

  assertThrows(
    () => test_error.throwSyntaxErrorCode(),
    {
      code: "ERR_TEST_CODE",
      message: "SyntaxError [syntax error]",
    },
  );

  let error = test_error.createError();
  assert(
    error instanceof Error,
    "expected error to be an instance of Error",
  );
  assertEquals(error.message, "error");

  error = test_error.createRangeError();
  assert(
    error instanceof RangeError,
    "expected error to be an instance of RangeError",
  );
  assertEquals(error.message, "range error");

  error = test_error.createTypeError();
  assert(
    error instanceof TypeError,
    "expected error to be an instance of TypeError",
  );
  assertEquals(error.message, "type error");

  // TODO(bartlomieju): this is experimental API
  // error = test_error.createSyntaxError();
  // assert(
  //   error instanceof SyntaxError,
  //   "expected error to be an instance of SyntaxError",
  // );
  // assertEquals(error.message, "syntax error");

  error = test_error.createErrorCode();
  assert(
    error instanceof Error,
    "expected error to be an instance of Error",
  );
  assertEquals(error.code, "ERR_TEST_CODE");
  assertEquals(error.message, "Error [error]");
  assertEquals(error.name, "Error");

  error = test_error.createRangeErrorCode();
  assert(
    error instanceof RangeError,
    "expected error to be an instance of RangeError",
  );
  assertEquals(error.message, "RangeError [range error]");
  assertEquals(error.code, "ERR_TEST_CODE");
  assertEquals(error.name, "RangeError");

  error = test_error.createTypeErrorCode();
  assert(
    error instanceof TypeError,
    "expected error to be an instance of TypeError",
  );
  assertEquals(error.message, "TypeError [type error]");
  assertEquals(error.code, "ERR_TEST_CODE");
  assertEquals(error.name, "TypeError");

  // TODO(bartlomieju): this is experimental API
  // error = test_error.createSyntaxErrorCode();
  // assert(
  //   error instanceof SyntaxError,
  //   "expected error to be an instance of SyntaxError",
  // );
  // assertEquals(error.message, "SyntaxError [syntax error]");
  // assertEquals(error.code, "ERR_TEST_CODE");
  // assertEquals(error.name, "SyntaxError");
});
