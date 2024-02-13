// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
es5id: 15.12.1.1-g6-7
description: >
    The JSON lexical grammar allows 't' as a JSONEscapeCharacter after
    '' in a JSONString
---*/

assert.sameValue(JSON.parse('"\\t"'), "\t", "JSON.parse('\"\\t\"')");
