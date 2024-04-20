// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, loadTestLibrary } from "./common.js";

const mem = loadTestLibrary();

Deno.test("napi adjust external memory", function () {
  const adjusted = mem.adjust_external_memory();
  assert(typeof adjusted === "number");
  assert(adjusted > 0);
});
