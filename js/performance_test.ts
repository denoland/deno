// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function now() {
  const start = performance.now();
  setTimeout(() => {
    const end = performance.now();
    assert(end - start >= 10);
  }, 10);
});
