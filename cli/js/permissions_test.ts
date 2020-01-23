// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEquals } from "./test_util.ts";

const knownPermissions: Deno.PermissionName[] = [
  "run",
  "read",
  "write",
  "net",
  "env",
  "plugin",
  "hrtime"
];

function genFunc(grant: Deno.PermissionName): () => Promise<void> {
  const gen: () => Promise<void> = async function Granted(): Promise<void> {
    const status0 = await Deno.permissions.query({ name: grant });
    assert(status0 != null);
    assertEquals(status0.state, "granted");

    const status1 = await Deno.permissions.revoke({ name: grant });
    assert(status1 != null);
    assertEquals(status1.state, "prompt");
  };
  // Properly name these generated functions.
  Object.defineProperty(gen, "name", { value: grant + "Granted" });
  return gen;
}

for (const grant of knownPermissions) {
  testPerm({ [grant]: true }, genFunc(grant));
}

test(async function permissionInvalidName(): Promise<void> {
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    await Deno.permissions.query({ name: "foo" as any });
  } catch (e) {
    assert(e.name === "Other");
  }
});

test(async function permissionNetInvalidUrl(): Promise<void> {
  try {
    await Deno.permissions.query({ name: "net", url: ":" });
  } catch (e) {
    assert(e.name === "UrlParse");
  }
});
