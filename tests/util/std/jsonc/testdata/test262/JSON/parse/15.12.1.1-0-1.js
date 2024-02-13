// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
es5id: 15.12.1.1-0-1
description: The JSON lexical grammar treats whitespace as a token separator
---*/

assert.throws(SyntaxError, function () {
  JSON.parse("12\t\r\n 34"); // should produce a syntax error as whitespace results in two tokens
});
