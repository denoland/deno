// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { DAY, HOUR, MINUTE, SECOND, WEEK } from "./constants.ts";

Deno.test({
  name: "[std/datetime] constants",
  fn() {
    assertEquals(SECOND, 1e3);
    assertEquals(MINUTE, SECOND * 60);
    assertEquals(HOUR, MINUTE * 60);
    assertEquals(DAY, HOUR * 24);
    assertEquals(WEEK, DAY * 7);
  },
});
