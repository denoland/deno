// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { grant, grantOrThrow } from "./mod.ts";
import { assert, assertEquals } from "../testing/asserts.ts";

const { test } = Deno;

test({
  name: "grant basic",
  async fn() {
    assertEquals(await grant({ name: "net" }, { name: "env" }), [
      { name: "net" },
      { name: "env" },
    ]);
  },
});

test({
  name: "grant array",
  async fn() {
    assertEquals(await grant([{ name: "net" }, { name: "env" }]), [
      { name: "net" },
      { name: "env" },
    ]);
  },
});

test({
  name: "grant logic",
  async fn() {
    assert(await grant({ name: "net" }));
  },
});

test({
  name: "grantOrThrow basic",
  async fn() {
    await grantOrThrow({ name: "net" }, { name: "env" });
  },
});

test({
  name: "grantOrThrow array",
  async fn() {
    await grantOrThrow([{ name: "net" }, { name: "env" }]);
  },
});
