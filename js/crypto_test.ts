// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert } from "./test_util.ts";

testPerm({}, function cryptoGetRandomValues(): void {
  const v = crypto.getRandomValues();
  console.log(`crypto.getRandomValues:${v}`);
  assert(typeof v === 'number');
  assert(Number.isInteger(v));
})
