// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

const testException = loadTestLibrary();

// NOTE(bartlomieju): In Node tests, when loading the binding it actually
// throws an exception and attaches actual bindings to `exception.binding`
// object. But I was too lazy to split this crate into multiple dylibs,
// so that part is skipped for now.

const theError = new Error("Some error");

Deno.test("napi exception1", function () {
  const throwTheError = () => {
    throw theError;
  };

  // Test that the native side successfully captures the exception
  let returnedError = testException.returnException(throwTheError);
  assertEquals(returnedError, theError);

  // Test that the native side passes the exception through
  let threw = false;
  try {
    testException.allowException(throwTheError);
  } catch (e) {
    threw = true;
    assertEquals(e, theError);
  } finally {
    assert(threw);
  }

  // Test that the exception thrown above was marked as pending
  // before it was handled on the JS side
  const exceptionPending = testException.wasPending();
  assertEquals(
    exceptionPending,
    true,
  );

  // Test that the native side does not capture a non-existing exception
  let callCount = 0;
  returnedError = testException.returnException(() => {
    callCount++;
  });
  assertEquals(callCount, 1);
  assertEquals(
    returnedError,
    undefined,
  );
});

Deno.test("napi exception2", function () {
  const throwTheError = class {
    constructor() {
      throw theError;
    }
  };

  let returnedError = testException.constructReturnException(throwTheError);
  assertEquals(returnedError, theError);

  // Test that the native side passes the exception through
  let threw = false;
  try {
    testException.constructAllowException(throwTheError);
  } catch (e) {
    threw = true;
    assertEquals(e, theError);
  } finally {
    assert(threw);
  }

  // Test that the exception thrown above was marked as pending
  // before it was handled on the JS side
  const exceptionPending = testException.wasPending();
  assertEquals(
    exceptionPending,
    true,
  );

  // Test that the native side does not capture a non-existing exception
  let callCount = 0;
  returnedError = testException.constructReturnException(() => {
    callCount++;
  });
  assertEquals(callCount, 1);
  assertEquals(
    returnedError,
    undefined,
  );
});

Deno.test("napi exception3", function () {
  let caughtError;
  let callCount = 0;
  let threw = false;
  try {
    testException.allowException(() => {
      callCount++;
    });
  } catch (anError) {
    threw = true;
    caughtError = anError;
  } finally {
    assert(threw);
  }
  assertEquals(callCount, 1);
  assertEquals(caughtError, undefined);

  const exceptionPending = testException.wasPending();
  assertEquals(exceptionPending, false);
});

Deno.test("napi exception4", function () {
  let caughtError;
  let callCount = 0;
  let threw = false;

  try {
    testException.constructAllowException(() => {
      callCount++;
    });
  } catch (anError) {
    threw = true;
    caughtError = anError;
  } finally {
    assert(threw);
  }
  assertEquals(callCount, 1);
  assertEquals(caughtError, undefined);

  const exceptionPending = testException.wasPending();
  assertEquals(exceptionPending, false);
});
