// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assert } from "../../testing/asserts.ts";
import { test } from "../../testing/mod.ts";
// @ts-ignore
import { NIL_UUID, isNil } from "../mod.ts";

test({
  name: "[UUID] isNil",
  fn(): void {
    const nil = NIL_UUID;
    const u = "582cbcff-dad6-4f28-888a-e062ae36bafc";
    assert(isNil(nil));
    assert(!isNil(u));
    console.log("");
  }
});
