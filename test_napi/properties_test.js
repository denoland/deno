// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const properties = loadTestLibrary();

Deno.test("napi properties", () => {
  properties.test_property_rw = 1;
  assertEquals(properties.test_property_rw, 1);
  properties.test_property_rw = 2;
  assertEquals(properties.test_property_rw, 2);

  // assertEquals(properties.test_property_r, 2);
  // assertRejects(() => properties.test_property_r = 3);
});
