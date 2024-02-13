// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
es5id: 15.12.1.1-g5-3
description: >
    A JSONStringCharacter UnicodeEscape may not include any non=hex
    characters
---*/

assert.throws(SyntaxError, function () {
  JSON.parse('"\\u0X50"');
});
