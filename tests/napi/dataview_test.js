// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

Deno.test("napi create_dataview and get_dataview_info", function () {
  const byteLength = lib.test_dataview();
  assertEquals(byteLength, 8);
});

Deno.test("napi is_dataview", function () {
  const result = lib.test_is_dataview();
  assertEquals(result, true);
});
