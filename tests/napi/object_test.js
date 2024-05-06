// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

const object = loadTestLibrary();

Deno.test("napi object", function () {
  const r = object.test_object_new(1, "hello");
  assertEquals(typeof r, "object");
  assertEquals(r[0], 1);
  assertEquals(r[1], "hello");

  const r1 = object.test_object_get(r);
  assert(r === r1);
});
