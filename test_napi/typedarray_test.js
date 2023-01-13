// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const typedarray = loadTestLibrary();

Deno.test("napi typedarray external", function () {
  assertEquals(
    new Uint8Array(typedarray.test_external()),
    new Uint8Array([0, 1, 2, 3]),
  );
});
