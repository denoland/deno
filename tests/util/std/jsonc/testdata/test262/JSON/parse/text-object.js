// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (C) 2019 Alexey Shvayka. All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.
/*---
esid: sec-json.parse
description: >
  Objects are coerced to strings using ToString.
info: |
  JSON.parse ( text [ , reviver ] )

  1. Let JText be ? ToString(text).
  2. Parse JText interpreted as UTF-16 encoded Unicode points (6.1.4) as a JSON
  text as specified in ECMA-404. Throw a SyntaxError exception if JText is not
  a valid JSON text as defined in that specification.
---*/

var hint = JSON.parse({
  toString: function () {
    return '"string"';
  },
  valueOf: function () {
    return '"default_or_number"';
  },
});

assert.sameValue(hint, "string");
