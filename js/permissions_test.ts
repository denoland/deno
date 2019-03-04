// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";
import { Permission } from "deno";

const knownPermissions: Permission[] = ["run", "read", "write", "net", "env"];

for (let grant of knownPermissions) {
  testPerm({ [grant]: true }, function envGranted() {
    const perms = Deno.permissions();
    assert(perms !== null);
    for (const perm in perms) {
      assertEqual(perms[perm], perm === grant);
    }

    Deno.revokePermission(grant);

    const revoked = Deno.permissions();
    for (const perm in revoked) {
      assertEqual(revoked[perm], false);
    }
  });
}
