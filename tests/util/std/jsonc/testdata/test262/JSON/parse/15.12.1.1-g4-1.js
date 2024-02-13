// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
es5id: 15.12.1.1-g4-1
description: >
    The JSON lexical grammar does not allow a JSONStringCharacter to
    be any of the Unicode characters U+0000 thru U+0007
---*/

assert.throws(SyntaxError, function () {
  JSON.parse('"\u0000\u0001\u0002\u0003\u0004\u0005\u0006\u0007"'); // invalid string characters should produce a syntax error
});
