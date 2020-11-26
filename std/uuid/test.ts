// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../testing/asserts.ts";
import { isNil, NIL_UUID } from "./mod.ts";

Deno.test({
  name: "[UUID] isNil",
  fn(): void {
    const nil = NIL_UUID;
    const u = "582cbcff-dad6-4f28-888a-e062ae36bafc";
    assert(isNil(nil));
    assert(!isNil(u));
  },
});
