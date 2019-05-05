// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
//
// We want to test many ops in deno which have different behavior depending on
// the permissions set. These tests can specify which permissions they expect,
// which appends a special string like "permW1N0" to the end of the test name.
// Here we run several copies of deno with different permissions, filtering the
// tests by the special string. permW1N0 means allow-write but not allow-net.
// See tools/unit_tests.py for more details.

import * as testing from "./deps/https/deno.land/std/testing/mod.ts";
import {
  assert,
  assertEquals
} from "./deps/https/deno.land/std/testing/asserts.ts";
export {
  assert,
  assertEquals
} from "./deps/https/deno.land/std/testing/asserts.ts";

interface TestPermissions {
  read?: boolean;
  write?: boolean;
  net?: boolean;
  env?: boolean;
  run?: boolean;
  highPrecision?: boolean;
}

const processPerms = Deno.permissions();

function permissionsMatch(
  processPerms: Deno.Permissions,
  requiredPerms: TestPermissions
): boolean {
  for (const permName in processPerms) {
    // if process has permission enabled and test case doesn't need this
    // perm then skip
    if (processPerms[permName] && !requiredPerms.hasOwnProperty(permName)) {
      return false;
    }

    // if test case requires permissions but process has different
    // value for perm then skip
    if (
      requiredPerms.hasOwnProperty(permName) &&
      requiredPerms[permName] !== processPerms[permName]
    ) {
      return false;
    }
  }

  return true;
}

export const permissionCombinations: Set<string> = new Set([]);

function registerPermCombination(perms: Deno.Permissions): void {
  // TODO: poor-man's set of unique objects, to be refactored
  permissionCombinations.add(JSON.stringify(perms));
}

function normalizeTestPermissions(perms: TestPermissions): Deno.Permissions {
  const normalizedPerms = {
    read: !!perms.read,
    write: !!perms.write,
    net: !!perms.net,
    run: !!perms.run,
    env: !!perms.env,
    highPrecision: !!perms.highPrecision
  };

  registerPermCombination(normalizedPerms);
  return normalizedPerms;
}

export function testPerm(
  perms: TestPermissions,
  fn: testing.TestFunction
): void {
  perms = normalizeTestPermissions(perms);

  if (!permissionsMatch(processPerms, perms)) {
    return;
  }

  testing.test(fn);
}

export function test(fn: testing.TestFunction): void {
  testPerm(
    {
      read: false,
      write: false,
      net: false,
      env: false,
      run: false,
      highPrecision: false
    },
    fn
  );
}

test(function permissionsMatches(): void {
  assert(
    permissionsMatch(
      {
        read: true,
        write: false,
        net: false,
        env: false,
        run: false,
        highPrecision: false
      },
      { read: true }
    )
  );

  assert(
    permissionsMatch(
      {
        read: false,
        write: false,
        net: false,
        env: false,
        run: false,
        highPrecision: false
      },
      {}
    )
  );

  assertEquals(
    permissionsMatch(
      {
        read: false,
        write: true,
        net: true,
        env: true,
        run: true,
        highPrecision: true
      },
      { read: true }
    ),
    false
  );

  assertEquals(
    permissionsMatch(
      {
        read: true,
        write: false,
        net: true,
        env: false,
        run: false,
        highPrecision: false
      },
      { read: true }
    ),
    false
  );

  assert(
    permissionsMatch(
      {
        read: true,
        write: true,
        net: true,
        env: true,
        run: true,
        highPrecision: true
      },
      {
        read: true,
        write: true,
        net: true,
        env: true,
        run: true,
        highPrecision: true
      }
    )
  );
});
