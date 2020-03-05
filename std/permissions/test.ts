// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { grant, grantOrThrow } from "./mod.ts";
import { assert } from "../testing/asserts.ts";

const { test } = Deno;

test({
  name: "grant basic",
  async fn() {
    assert(await grant({ name: "net" }));
  }
});

test({
  name: "grantOrThrow basic",
  async fn() {
    await grantOrThrow({ name: "net" });
  }
});
