// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, loadTestLibrary } from "./common.js";

const mem = loadTestLibrary();

Deno.test("napi adjust external memory", function () {
  const adjusted = mem.adjust_external_memory();
  assert(typeof adjusted === "number");
  assert(adjusted > 0);
});
