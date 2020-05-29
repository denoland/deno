// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../testing/asserts.ts";
const { test } = Deno;
import { NIL_UUID, isNil } from "../mod.ts";

test({
  name: "[UUID] isNil",
  fn(): void {
    const nil = NIL_UUID;
    const u = "582cbcff-dad6-4f28-888a-e062ae36bafc";
    assert(isNil(nil));
    assert(!isNil(u));
  },
});
