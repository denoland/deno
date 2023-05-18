// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, loadTestLibrary } from "./common.js";

const env = loadTestLibrary();

Deno.test("napi get global", function () {
  const g = env.testNodeGlobal();
  // Note: global is a mock object in the tests.
  // See common.js
  assert(g.Buffer);
});
