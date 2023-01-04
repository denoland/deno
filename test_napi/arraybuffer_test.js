// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const typedarray = loadTestLibrary();

Deno.test("napi arraybuffer detach", function () {
  const buf = new ArrayBuffer(5);
  assertEquals(buf.byteLength, 5);
  typedarray.test_detached(buf);
  assertEquals(buf.byteLength, 0);
});
