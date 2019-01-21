// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function globalThisExists() {
  assert(globalThis != null);
});

test(function windowExists() {
  assert(window != null);
});

test(function windowWindowExists() {
  assert(window.window === window);
});

test(function globalThisEqualsWindow() {
  assert(globalThis === window);
});
