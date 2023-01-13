// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any

import { assert } from "./test_util.ts";

Deno.test({ permissions: "none" }, function webStoragesReassignable() {
  // Can reassign to web storages
  globalThis.localStorage = 1 as any;
  globalThis.sessionStorage = 1 as any;
  // The actual values don't change
  assert(globalThis.localStorage instanceof globalThis.Storage);
  assert(globalThis.sessionStorage instanceof globalThis.Storage);
});
