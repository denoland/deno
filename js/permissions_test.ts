// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";
import { Permission } from "deno";

const perms: Permission[] = [
  "run",
  "read",
  "write",
  "net",
  "env",
];

for (let grant of perms) {
  testPerm({ [grant]: true }, function envGranted() {

    const perms = Deno.permissions();
    assert(perms !== null);
    for (let perm of Object.keys(perms)) {
      assertEqual(perms[perm], perm === grant);
    }

    Deno.revokePermission(grant);

    const revoked = Deno.permissions();
    for (let perm of Object.keys(revoked)) {
      assertEqual(revoked[perm], false);
    }
  });
}
