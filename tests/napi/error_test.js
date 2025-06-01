// Copyright 2018-2025 the Deno authors. MIT license.

import {
  assert,
  assertEquals,
  assertThrows,
  loadTestLibrary,
} from "./common.js";

const testError = loadTestLibrary();

const theError = new Error("Some error");
const theTypeError = new TypeError("Some type error");
const theSyntaxError = new SyntaxError("Some syntax error");
const theRangeError = new RangeError("Some type error");
const theReferenceError = new ReferenceError("Some reference error");
const theURIError = new URIError("Some URI error");
const theEvalError = new EvalError("Some eval error");

function assertThrowsWithCode(fn, value) {
  let thrown = false;

  try {
    fn();
  } catch (e) {
    thrown = true;
    assertEquals(e.message, value.message);
    assertEquals(e.code, value.code);
  } finally {
    assert(thrown);
  }
}

Deno.test("napi error", function () {
  class MyError extends Error {}
  const myError = new MyError("Some MyError");

  // Test that native error object is correctly classed
  assertEquals(testError.checkError(theError), true);

  // Test that native type error object is correctly classed
  assertEquals(testError.checkError(theTypeError), true);

  // Test that native syntax error object is correctly classed
  assertEquals(testError.checkError(theSyntaxError), true);

  // Test that native range error object is correctly classed
  assertEquals(testError.checkError(theRangeError), true);

  // Test that native reference error object is correctly classed
  assertEquals(testError.checkError(theReferenceError), true);

  // Test that native URI error object is correctly classed
  assertEquals(testError.checkError(theURIError), true);

  // Test that native eval error object is correctly classed
  assertEquals(testError.checkError(theEvalError), true);

  // Test that class derived from native error is correctly classed
  assertEquals(testError.checkError(myError), true);

  // Test that non-error object is correctly classed
  assertEquals(testError.checkError({}), false);

  // Test that non-error primitive is correctly classed
  assertEquals(testError.checkError("non-object"), false);

  assertThrows(
    () => {
      testError.throwExistingError();
    },
    Error,
    "existing error",
  );

  assertThrows(
    () => {
      testError.throwError();
    },
    Error,
    "error",
  );

  assertThrows(
    () => {
      testError.throwRangeError();
    },
    RangeError,
    "range error",
  );

  assertThrows(
    () => {
      testError.throwTypeError();
    },
    TypeError,
    "type error",
  );

  // assertThrows(() => {
  //   testError.throwSyntaxError();
  // }, "SyntaxError: syntax error");

  [42, {}, [], Symbol("xyzzy"), true, "ball", undefined, null, NaN]
    .forEach((value) => {
      let thrown = false;

      try {
        testError.throwArbitrary(value);
      } catch (e) {
        thrown = true;
        assertEquals(e, value);
      } finally {
        assert(thrown);
      }
    });

  assertThrowsWithCode(
    () => testError.throwErrorCode(),
    {
      code: "ERR_TEST_CODE",
      message: "Error [error]",
    },
  );

  assertThrowsWithCode(
    () => testError.throwRangeErrorCode(),
    {
      code: "ERR_TEST_CODE",
      message: "RangeError [range error]",
    },
  );

  assertThrowsWithCode(
    () => testError.throwTypeErrorCode(),
    {
      code: "ERR_TEST_CODE",
      message: "TypeError [type error]",
    },
  );

  // assertThrowsWithCode(
  //   () => testError.throwSyntaxErrorCode(),
  //   {
  //     code: "ERR_TEST_CODE",
  //     message: "SyntaxError [syntax error]",
  //   },
  // );

  let error = testError.createError();
  assert(
    error instanceof Error,
    "expected error to be an instance of Error",
  );
  assertEquals(error.message, "error");

  error = testError.createRangeError();
  assert(
    error instanceof RangeError,
    "expected error to be an instance of RangeError",
  );
  assertEquals(error.message, "range error");

  error = testError.createTypeError();
  assert(
    error instanceof TypeError,
    "expected error to be an instance of TypeError",
  );
  assertEquals(error.message, "type error");

  // TODO(bartlomieju): this is experimental API
  // error = testError.createSyntaxError();
  // assert(
  //   error instanceof SyntaxError,
  //   "expected error to be an instance of SyntaxError",
  // );
  // assertEquals(error.message, "syntax error");

  error = testError.createErrorCode();
  assert(
    error instanceof Error,
    "expected error to be an instance of Error",
  );
  assertEquals(error.code, "ERR_TEST_CODE");
  assertEquals(error.message, "Error [error]");
  assertEquals(error.name, "Error");

  error = testError.createRangeErrorCode();
  assert(
    error instanceof RangeError,
    "expected error to be an instance of RangeError",
  );
  assertEquals(error.message, "RangeError [range error]");
  assertEquals(error.code, "ERR_TEST_CODE");
  assertEquals(error.name, "RangeError");

  error = testError.createTypeErrorCode();
  assert(
    error instanceof TypeError,
    "expected error to be an instance of TypeError",
  );
  assertEquals(error.message, "TypeError [type error]");
  assertEquals(error.code, "ERR_TEST_CODE");
  assertEquals(error.name, "TypeError");

  // TODO(bartlomieju): this is experimental API
  // error = testError.createSyntaxErrorCode();
  // assert(
  //   error instanceof SyntaxError,
  //   "expected error to be an instance of SyntaxError",
  // );
  // assertEquals(error.message, "SyntaxError [syntax error]");
  // assertEquals(error.code, "ERR_TEST_CODE");
  // assertEquals(error.name, "SyntaxError");
});
