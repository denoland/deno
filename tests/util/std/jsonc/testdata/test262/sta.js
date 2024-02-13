// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Ported from test262
// https://github.com/tc39/test262/blob/488eb365db7c613d52e72a9f5b8726684906e540/harness/sta.js
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.
/*---
description: |
    Provides both:
    - An error class to avoid false positives when testing for thrown exceptions
    - A function to explicitly throw an exception using the Test262Error class
defines: [Test262Error, $ERROR, $DONOTEVALUATE]
---*/

function Test262Error(message) {
  this.message = message || "";
}

Test262Error.prototype.toString = function () {
  return "Test262Error: " + this.message;
};

Test262Error.thrower = (message) => {
  throw new Test262Error(message);
};
// TODO: Remove when $ERROR migration is completed
var $ERROR = Test262Error.thrower;

function $DONOTEVALUATE() {
  throw "Test262: This statement should not be evaluated.";
}
