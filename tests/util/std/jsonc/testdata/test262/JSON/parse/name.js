// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (C) 2015 André Bargull. All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
esid: sec-json.parse
description: >
  JSON.parse.name is "parse".
info: |
  JSON.parse ( text [ , reviver ] )

  17 ECMAScript Standard Built-in Objects:
    Every built-in Function object, including constructors, that is not
    identified as an anonymous function has a name property whose value
    is a String.

    Unless otherwise specified, the name property of a built-in Function
    object, if it exists, has the attributes { [[Writable]]: false,
    [[Enumerable]]: false, [[Configurable]]: true }.
includes: [propertyHelper.js]
---*/

verifyProperty(JSON.parse, "name", {
  value: "parse",
  writable: false,
  enumerable: false,
  configurable: true,
});
