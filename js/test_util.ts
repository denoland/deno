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
  requiredPerms: Deno.Permissions
): boolean {
  for (const permName in processPerms) {
    if (processPerms[permName] !== requiredPerms[permName]) {
      return false;
    }
  }

  return true;
}

export const permissionCombinations: Map<string, Deno.Permissions> = new Map();

function permToString(perms: Deno.Permissions): string {
  const r = perms.read ? 1 : 0;
  const w = perms.write ? 1 : 0;
  const n = perms.net ? 1 : 0;
  const e = perms.env ? 1 : 0;
  const u = perms.run ? 1 : 0;
  const h = perms.highPrecision ? 1 : 0;
  return `permR${r}W${w}N${n}E${e}U${u}H${h}`;
}

function registerPermCombination(perms: Deno.Permissions): void {
  const key = permToString(perms);
  if (!permissionCombinations.has(key)) {
    permissionCombinations.set(key, perms);
  }
}

function normalizeTestPermissions(perms: TestPermissions): Deno.Permissions {
  return {
    read: !!perms.read,
    write: !!perms.write,
    net: !!perms.net,
    run: !!perms.run,
    env: !!perms.env,
    highPrecision: !!perms.highPrecision
  };
}

export function testPerm(
  perms: TestPermissions,
  fn: testing.TestFunction
): void {
  const normalizedPerms = normalizeTestPermissions(perms);

  registerPermCombination(normalizedPerms);

  if (!permissionsMatch(processPerms, normalizedPerms)) {
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
      normalizeTestPermissions({ read: true })
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
      normalizeTestPermissions({})
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
      normalizeTestPermissions({ read: true })
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
      normalizeTestPermissions({ read: true })
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
