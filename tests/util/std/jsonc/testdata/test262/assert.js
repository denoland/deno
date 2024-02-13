// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Ported from test262
// https://github.com/tc39/test262/blob/488eb365db7c613d52e72a9f5b8726684906e540/harness/assert.js
// Copyright (C) 2017 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.
/*---
description: |
    Collection of assertion functions used throughout test262
defines: [assert]
---*/

function assert(mustBeTrue, message) {
  if (mustBeTrue === true) {
    return;
  }

  if (message === undefined) {
    message = "Expected true but got " + assert._toString(mustBeTrue);
  }
  throw new Test262Error(message);
}

assert._isSameValue = function (a, b) {
  if (a === b) {
    // Handle +/-0 vs. -/+0
    return a !== 0 || 1 / a === 1 / b;
  }

  // Handle NaN vs. NaN
  return a !== a && b !== b;
};

assert.sameValue = function (actual, expected, message) {
  try {
    if (assert._isSameValue(actual, expected)) {
      return;
    }
  } catch (error) {
    throw new Test262Error(
      message + " (_isSameValue operation threw) " + error,
    );
    return;
  }

  if (message === undefined) {
    message = "";
  } else {
    message += " ";
  }

  message += "Expected SameValue(«" + assert._toString(actual) + "», «" +
    assert._toString(expected) + "») to be true";

  throw new Test262Error(message);
};

assert.notSameValue = function (actual, unexpected, message) {
  if (!assert._isSameValue(actual, unexpected)) {
    return;
  }

  if (message === undefined) {
    message = "";
  } else {
    message += " ";
  }

  message += "Expected SameValue(«" + assert._toString(actual) + "», «" +
    assert._toString(unexpected) + "») to be false";

  throw new Test262Error(message);
};

assert.throws = function (expectedErrorConstructor, func, message) {
  var expectedName, actualName;
  if (typeof func !== "function") {
    throw new Test262Error(
      "assert.throws requires two arguments: the error constructor " +
        "and a function to run",
    );
    return;
  }
  if (message === undefined) {
    message = "";
  } else {
    message += " ";
  }

  try {
    func();
  } catch (thrown) {
    if (typeof thrown !== "object" || thrown === null) {
      message += "Thrown value was not an object!";
      throw new Test262Error(message);
    } else if (thrown.constructor !== expectedErrorConstructor) {
      expectedName = expectedErrorConstructor.name;
      actualName = thrown.constructor.name;
      if (expectedName === actualName) {
        message += "Expected a " + expectedName +
          " but got a different error constructor with the same name";
      } else {
        message += "Expected a " + expectedName + " but got a " + actualName;
      }
      throw new Test262Error(message);
    }
    return;
  }

  message += "Expected a " + expectedErrorConstructor.name +
    " to be thrown but no exception was thrown at all";
  throw new Test262Error(message);
};

assert._toString = function (value) {
  try {
    if (value === 0 && 1 / value === -Infinity) {
      return "-0";
    }

    return String(value);
  } catch (err) {
    if (err.name === "TypeError") {
      return Object.prototype.toString.call(value);
    }

    throw err;
  }
};
