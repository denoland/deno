// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

const properties = loadTestLibrary();

Deno.test("napi properties", () => {
  assertEquals(properties.test_property_rw, 1);
  properties.test_property_rw = 2;
  assertEquals(properties.test_property_rw, 2);

  assertEquals(properties.test_property_r, 1);

  // https://github.com/denoland/deno/issues/17509
  assertEquals(properties.test_simple_property, {
    nice: 69,
  });
});
