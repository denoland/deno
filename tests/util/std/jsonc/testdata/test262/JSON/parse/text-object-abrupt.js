// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (C) 2019 Alexey Shvayka. All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.
/*---
esid: sec-json.parse
description: >
  Abrupt completion from Get and Call in ToPrimitive.
info: |
  JSON.parse ( text [ , reviver ] )

  1. Let JText be ? ToString(text).
---*/

assert.throws(Test262Error, function () {
  JSON.parse({
    toString: null,
    get valueOf() {
      throw new Test262Error();
    },
  });
});

assert.throws(Test262Error, function () {
  JSON.parse({
    toString: function () {
      throw new Test262Error();
    },
  });
});
