// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { Buffer } from "node:buffer";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

Deno.test({
  name: "[node/buffer] slice with infinity returns empty buffer",
  fn() {
    const buf = Buffer.from([1, 2, 3, 4, 5]);
    assertEquals(buf.slice(Infinity).length, 0);
  },
});
