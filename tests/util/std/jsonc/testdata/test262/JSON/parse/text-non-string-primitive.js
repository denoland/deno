// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (C) 2019 Alexey Shvayka. All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.
/*---
esid: sec-json.parse
description: >
  Primitive values are coerced to strings and parsed.
info: |
  JSON.parse ( text [ , reviver ] )

  1. Let JText be ? ToString(text).
  2. Parse JText interpreted as UTF-16 encoded Unicode points (6.1.4) as a JSON
  text as specified in ECMA-404. Throw a SyntaxError exception if JText is not
  a valid JSON text as defined in that specification.
features: [Symbol]
---*/

assert.throws(SyntaxError, function () {
  JSON.parse();
});

assert.throws(SyntaxError, function () {
  JSON.parse(undefined);
});

assert.sameValue(JSON.parse(null), null);
assert.sameValue(JSON.parse(false), false);
assert.sameValue(JSON.parse(true), true);
assert.sameValue(JSON.parse(0), 0);
assert.sameValue(JSON.parse(3.14), 3.14);

var sym = Symbol("desc");
assert.throws(TypeError, function () {
  JSON.parse(sym);
});
