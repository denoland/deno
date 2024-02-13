// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
es5id: 15.12.1.1-0-8
description: >
    U+2028 and U+2029 are not valid JSON whitespace as specified by
    the production JSONWhitespace.
---*/

assert.throws(SyntaxError, function () {
  JSON.parse("\u2028\u20291234"); // should produce a syntax error
});
