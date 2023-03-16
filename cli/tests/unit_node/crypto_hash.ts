// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { createHash, createHmac } from "node:crypto";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

// https://github.com/denoland/deno/issues/18140
Deno.test({
  name: "createHmac digest",
  fn() {
    assertEquals(
      createHmac("sha256", "secret").update("hello").digest("hex"),
      "88aab3ede8d3adf94d26ab90d3bafd4a2083070c3bcce9c014ee04a443847c0b",
    );
  },
});

Deno.test({
  name: "createHash digest",
  fn() {
    assertEquals(
      createHash("sha256").update("hello").digest("hex"),
      "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
    );
  },
});
