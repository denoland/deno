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

function genFunc(grant: Deno.PermissionName): [string, () => Promise<void>] {
  const gen: () => Promise<void> = async function Granted(): Promise<void> {
    const status0 = await Deno.permissions.query({ name: grant });
    assert(status0 != null);
    assert(status0.state === "granted");

    const status1 = await Deno.permissions.revoke({ name: grant });
    assert(status1 != null);
    assert(status1.state === "prompt");
  };
  const name = grant + "Granted";
  return [name, gen];
}

for (const grant of knownPermissions) {
  const [name, fn] = genFunc(grant);
  Deno.test(name, fn);
}
