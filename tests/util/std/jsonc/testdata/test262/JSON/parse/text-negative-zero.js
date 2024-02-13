// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (C) 2019 Alexey Shvayka. All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.
/*---
esid: sec-json.parse
description: >
  Top-level negative zero surrounded by whitespace is parsed correctly.
info: |
  JSON.parse ( text [ , reviver ] )

  1. Let JText be ? ToString(text).
  2. Parse JText interpreted as UTF-16 encoded Unicode points (6.1.4) as a JSON
  text as specified in ECMA-404. Throw a SyntaxError exception if JText is not
  a valid JSON text as defined in that specification.
---*/

assert.sameValue(JSON.parse("-0"), -0);
assert.sameValue(JSON.parse(" \n-0"), -0);
assert.sameValue(JSON.parse("-0  \t"), -0);
assert.sameValue(JSON.parse("\n\t -0\n   "), -0);

assert.sameValue(JSON.parse(-0), 0);
