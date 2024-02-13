// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2011 the Sputnik authors.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
info: JSON.parse must create a property with the given property name
es5id: 15.12.2_A1
description: Tests that JSON.parse treats "__proto__" as a regular property name
---*/

var x = JSON.parse('{"__proto__":[]}');

assert.sameValue(
  Object.getPrototypeOf(x),
  Object.prototype,
  'Object.getPrototypeOf("JSON.parse(\'{"__proto__":[]}\')") returns Object.prototype',
);

assert(
  Array.isArray(x.__proto__),
  "Array.isArray(x.__proto__) must return true",
);
