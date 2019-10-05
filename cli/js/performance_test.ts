// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert } from "./test_util.ts";

testPerm({ hrtime: false }, function now(): void {
  const start = performance.now();
  setTimeout((): void => {
    const end = performance.now();
    assert(end - start >= 10);
  }, 10);
});
