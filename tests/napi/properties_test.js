// Copyright 2018-2025 the Deno authors. MIT license.

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

  assertEquals(properties.key_v8_string, 1);
  const symbols = Object.getOwnPropertySymbols(properties);
  assertEquals(symbols.length, 1);
  assertEquals(symbols[0].description, "key_v8_symbol");
  assertEquals(properties[symbols[0]], 1);
});
