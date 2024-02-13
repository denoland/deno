// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (c) 2012 Ecma International.  All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
es5id: 15.12.1.1-g5-1
description: >
    The JSON lexical grammar allows Unicode escape sequences in a
    JSONString
---*/

assert.sameValue(JSON.parse('"\\u0058"'), "X", "JSON.parse('\"\\u0058\"')");
