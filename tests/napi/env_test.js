// Copyright 2018-2025 the Deno authors. MIT license.

import { assert, loadTestLibrary } from "./common.js";

const env = loadTestLibrary();

Deno.test("napi get global", function () {
  const g = env.testNodeGlobal();
  assert(g === globalThis);
});
