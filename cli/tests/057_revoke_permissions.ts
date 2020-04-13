// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

const knownPermissions: Deno.PermissionName[] = [
  "run",
  "read",
  "write",
  "net",
  "env",
  "plugin",
  "hrtime",
];

export function assert(cond: unknown): asserts cond {
  if (!cond) {
    throw Error("Assertion failed");
  }
}

function genFunc(grant: Deno.PermissionName): () => Promise<void> {
  const gen: () => Promise<void> = async function Granted(): Promise<void> {
    const status0 = await Deno.permissions.query({ name: grant });
    assert(status0 != null);
    assert(status0.state === "granted");

    const status1 = await Deno.permissions.revoke({ name: grant });
    assert(status1 != null);
    assert(status1.state === "prompt");
  };
  // Properly name these generated functions.
  Object.defineProperty(gen, "name", { value: grant + "Granted" });
  return gen;
}

for (const grant of knownPermissions) {
  Deno.test(genFunc(grant));
}
