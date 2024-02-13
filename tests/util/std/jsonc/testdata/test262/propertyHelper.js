// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Ported from test262
// https://github.com/tc39/test262/blob/276e79d62e8c45bc1e427fc680320c4899eace27/harness/propertyHelper.js
// Copyright (C) 2017 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.
/*---
description: |
    Collection of functions used to safely verify the correctness of
    property descriptors.
defines:
  - verifyProperty
  - verifyEqualTo
  - verifyWritable
  - verifyNotWritable
  - verifyEnumerable
  - verifyNotEnumerable
  - verifyConfigurable
  - verifyNotConfigurable
---*/

// @ts-check

/**
 * @param {object} obj
 * @param {string|symbol} name
 * @param {PropertyDescriptor|undefined} desc
 * @param {object} [options]
 * @param {boolean} [options.restore]
 */
function verifyProperty(obj, name, desc, options) {
  assert(
    arguments.length > 2,
    "verifyProperty should receive at least 3 arguments: obj, name, and descriptor",
  );

  var originalDesc = Object.getOwnPropertyDescriptor(obj, name);
  var nameStr = String(name);

  // Allows checking for undefined descriptor if it's explicitly given.
  if (desc === undefined) {
    assert.sameValue(
      originalDesc,
      undefined,
      "obj['" + nameStr + "'] descriptor should be undefined",
    );

    // desc and originalDesc are both undefined, problem solved;
    return true;
  }

  assert(
    Object.prototype.hasOwnProperty.call(obj, name),
    "obj should have an own property " + nameStr,
  );

  assert.notSameValue(
    desc,
    null,
    "The desc argument should be an object or undefined, null",
  );

  assert.sameValue(
    typeof desc,
    "object",
    "The desc argument should be an object or undefined, " + String(desc),
  );

  var failures = [];

  if (Object.prototype.hasOwnProperty.call(desc, "value")) {
    if (!isSameValue(desc.value, originalDesc.value)) {
      failures.push("descriptor value should be " + desc.value);
    }
  }

  if (Object.prototype.hasOwnProperty.call(desc, "enumerable")) {
    if (
      desc.enumerable !== originalDesc.enumerable ||
      desc.enumerable !== isEnumerable(obj, name)
    ) {
      failures.push(
        "descriptor should " + (desc.enumerable ? "" : "not ") +
          "be enumerable",
      );
    }
  }

  if (Object.prototype.hasOwnProperty.call(desc, "writable")) {
    if (
      desc.writable !== originalDesc.writable ||
      desc.writable !== isWritable(obj, name)
    ) {
      failures.push(
        "descriptor should " + (desc.writable ? "" : "not ") + "be writable",
      );
    }
  }

  if (Object.prototype.hasOwnProperty.call(desc, "configurable")) {
    if (
      desc.configurable !== originalDesc.configurable ||
      desc.configurable !== isConfigurable(obj, name)
    ) {
      failures.push(
        "descriptor should " + (desc.configurable ? "" : "not ") +
          "be configurable",
      );
    }
  }

  assert(!failures.length, failures.join("; "));

  if (options && options.restore) {
    Object.defineProperty(obj, name, originalDesc);
  }

  return true;
}

function isConfigurable(obj, name) {
  var hasOwnProperty = Object.prototype.hasOwnProperty;
  try {
    delete obj[name];
  } catch (e) {
    if (!(e instanceof TypeError)) {
      $ERROR("Expected TypeError, got " + e);
    }
  }
  return !hasOwnProperty.call(obj, name);
}

function isEnumerable(obj, name) {
  var stringCheck = false;

  if (typeof name === "string") {
    for (var x in obj) {
      if (x === name) {
        stringCheck = true;
        break;
      }
    }
  } else {
    // skip it if name is not string, works for Symbol names.
    stringCheck = true;
  }

  return stringCheck &&
    Object.prototype.hasOwnProperty.call(obj, name) &&
    Object.prototype.propertyIsEnumerable.call(obj, name);
}

function isSameValue(a, b) {
  if (a === 0 && b === 0) return 1 / a === 1 / b;
  if (a !== a && b !== b) return true;

  return a === b;
}

var __isArray = Array.isArray;
function isWritable(obj, name, verifyProp, value) {
  var unlikelyValue = __isArray(obj) && name === "length"
    ? Math.pow(2, 32) - 1
    : "unlikelyValue";
  var newValue = value || unlikelyValue;
  var hadValue = Object.prototype.hasOwnProperty.call(obj, name);
  var oldValue = obj[name];
  var writeSucceeded;

  try {
    obj[name] = newValue;
  } catch (e) {
    if (!(e instanceof TypeError)) {
      $ERROR("Expected TypeError, got " + e);
    }
  }

  writeSucceeded = isSameValue(obj[verifyProp || name], newValue);

  // Revert the change only if it was successful (in other cases, reverting
  // is unnecessary and may trigger exceptions for certain property
  // configurations)
  if (writeSucceeded) {
    if (hadValue) {
      obj[name] = oldValue;
    } else {
      delete obj[name];
    }
  }

  return writeSucceeded;
}

function verifyEqualTo(obj, name, value) {
  if (!isSameValue(obj[name], value)) {
    $ERROR(
      "Expected obj[" + String(name) + "] to equal " + value +
        ", actually " + obj[name],
    );
  }
}

function verifyWritable(obj, name, verifyProp, value) {
  if (!verifyProp) {
    assert(
      Object.getOwnPropertyDescriptor(obj, name).writable,
      "Expected obj[" + String(name) + "] to have writable:true.",
    );
  }
  if (!isWritable(obj, name, verifyProp, value)) {
    $ERROR("Expected obj[" + String(name) + "] to be writable, but was not.");
  }
}

function verifyNotWritable(obj, name, verifyProp, value) {
  if (!verifyProp) {
    assert(
      !Object.getOwnPropertyDescriptor(obj, name).writable,
      "Expected obj[" + String(name) + "] to have writable:false.",
    );
  }
  if (isWritable(obj, name, verifyProp)) {
    $ERROR("Expected obj[" + String(name) + "] NOT to be writable, but was.");
  }
}

function verifyEnumerable(obj, name) {
  assert(
    Object.getOwnPropertyDescriptor(obj, name).enumerable,
    "Expected obj[" + String(name) + "] to have enumerable:true.",
  );
  if (!isEnumerable(obj, name)) {
    $ERROR("Expected obj[" + String(name) + "] to be enumerable, but was not.");
  }
}

function verifyNotEnumerable(obj, name) {
  assert(
    !Object.getOwnPropertyDescriptor(obj, name).enumerable,
    "Expected obj[" + String(name) + "] to have enumerable:false.",
  );
  if (isEnumerable(obj, name)) {
    $ERROR("Expected obj[" + String(name) + "] NOT to be enumerable, but was.");
  }
}

function verifyConfigurable(obj, name) {
  assert(
    Object.getOwnPropertyDescriptor(obj, name).configurable,
    "Expected obj[" + String(name) + "] to have configurable:true.",
  );
  if (!isConfigurable(obj, name)) {
    $ERROR(
      "Expected obj[" + String(name) + "] to be configurable, but was not.",
    );
  }
}

function verifyNotConfigurable(obj, name) {
  assert(
    !Object.getOwnPropertyDescriptor(obj, name).configurable,
    "Expected obj[" + String(name) + "] to have configurable:false.",
  );
  if (isConfigurable(obj, name)) {
    $ERROR(
      "Expected obj[" + String(name) + "] NOT to be configurable, but was.",
    );
  }
}
