// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertNotInstanceOf } from "./mod.ts";

Deno.test({
  name: "assertNotInstanceOf",
  fn() {
    assertNotInstanceOf("not a number", Number);
    assertNotInstanceOf(42, String);
    assertNotInstanceOf(new URL("http://example.com"), Boolean);
  },
});
