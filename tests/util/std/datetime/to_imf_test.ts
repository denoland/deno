// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { toIMF } from "./to_imf.ts";

Deno.test({
  name: "[std/datetime] to IMF",
  fn() {
    const actual = toIMF(new Date(Date.UTC(1994, 3, 5, 15, 32)));
    const expected = "Tue, 05 Apr 1994 15:32:00 GMT";
    assertEquals(actual, expected);
  },
});

Deno.test({
  name: "[std/datetime] to IMF 0",
  fn() {
    const actual = toIMF(new Date(0));
    const expected = "Thu, 01 Jan 1970 00:00:00 GMT";
    assertEquals(actual, expected);
  },
});
