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

interface DenoPermissions {
  read?: boolean;
  write?: boolean;
  net?: boolean;
  env?: boolean;
  run?: boolean;
  highPrecision?: boolean;
}

function permToString(perms: DenoPermissions): string {
  const r = perms.read ? 1 : 0;
  const w = perms.write ? 1 : 0;
  const n = perms.net ? 1 : 0;
  const e = perms.env ? 1 : 0;
  const u = perms.run ? 1 : 0;
  const h = perms.highPrecision ? 1 : 0;
  return `permR${r}W${w}N${n}E${e}U${u}H${h}`;
}

function permFromString(s: string): DenoPermissions {
  const re = /^permR([01])W([01])N([01])E([01])U([01])H([01])$/;
  const found = s.match(re);
  if (!found) {
    throw Error("Not a permission string");
  }
  return {
    read: Boolean(Number(found[1])),
    write: Boolean(Number(found[2])),
    net: Boolean(Number(found[3])),
    env: Boolean(Number(found[4])),
    run: Boolean(Number(found[5])),
    highPrecision: Boolean(Number(found[6]))
  };
}

const processPerms = Deno.permissions();

function permissionsMatch(
  processPerms: Deno.Permissions,
  requiredPerms: DenoPermissions
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

export function testPerm(
  perms: DenoPermissions,
  fn: testing.TestFunction
): void {
  if (!permissionsMatch(processPerms, perms)) {
    return;
  }

  const name = `${fn.name}_${permToString(perms)}`;
  testing.test({ fn, name });
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

test(function permSerialization(): void {
  for (const write of [true, false]) {
    for (const net of [true, false]) {
      for (const env of [true, false]) {
        for (const run of [true, false]) {
          for (const read of [true, false]) {
            for (const highPrecision of [true, false]) {
              const perms: DenoPermissions = {
                write,
                net,
                env,
                run,
                read,
                highPrecision
              };
              assertEquals(perms, permFromString(permToString(perms)));
            }
          }
        }
      }
    }
  }
});

test(function permsMatchesOk(): void {
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
// To better catch internal errors, permFromString should throw if it gets an
// invalid permission string.
test(function permFromStringThrows(): void {
  let threw = false;
  try {
    permFromString("bad");
  } catch (e) {
    threw = true;
  }
  assert(threw);
});
