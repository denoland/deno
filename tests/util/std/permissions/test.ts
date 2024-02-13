// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { grant, grantOrThrow } from "./mod.ts";
import { assert, assertEquals, assertRejects } from "../assert/mod.ts";

Deno.test({
  name: "grant basic",
  async fn() {
    assertEquals(await grant({ name: "net" }, { name: "env" }), [
      { name: "net" },
      { name: "env" },
    ]);
  },
});

Deno.test({
  name: "grant array",
  async fn() {
    assertEquals(await grant([{ name: "net" }, { name: "env" }]), [
      { name: "net" },
      { name: "env" },
    ]);
  },
});

Deno.test({
  name: "grant logic",
  async fn() {
    assert(await grant({ name: "net" }));
  },
});

Deno.test({
  name: "grantOrThrow basic",
  async fn() {
    await grantOrThrow({ name: "net" }, { name: "env" });
  },
});

Deno.test({
  name: "grantOrThrow array",
  async fn() {
    await grantOrThrow([{ name: "net" }, { name: "env" }]);
  },
});

Deno.test({
  name: "grantOrThrow invalid argument",
  async fn() {
    await assertRejects(
      () => {
        return grantOrThrow();
      },
      TypeError,
      `The provided value "undefined" is not a valid permission name.`,
    );
  },
});

Deno.test({
  name: "grantOrThrow invalid permissionDescriptor name",
  async fn() {
    await assertRejects(
      () => {
        // deno-lint-ignore no-explicit-any
        return grantOrThrow({ name: "nett" } as any);
      },
      TypeError,
      'The provided value "nett" is not a valid permission name',
    );
  },
});

// TODO(wafuwafu13): Add denied case
