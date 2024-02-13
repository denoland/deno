// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
es5id: 15.12.1.1-g1-1
description: The JSON lexical grammar treats <TAB> as a whitespace character
---*/

assert.sameValue(JSON.parse("\t1234"), 1234, "<TAB> should be ignored");

assert.throws(SyntaxError, function () {
  JSON.parse("12\t34");
}, "<TAB> should produce a syntax error as whitespace results in two tokens");
