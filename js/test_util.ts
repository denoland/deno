// Copyright 2018 the Deno authors. All rights reserved. MIT license.
//
// We want to test many ops in deno which have different behavior depending on
// the permissions set. These tests can specify which permissions they expect,
// which appends a special string like "permW1N0" to the end of the test name.
// Here we run several copies of deno with different permissions, filtering the
// tests by the special string. permW0N0 means allow-write but not allow-net.
// See tools/unit_tests.py for more details.

import * as deno from "deno";
import * as testing from "./testing/testing.ts";
export { assert, assertEqual } from "./testing/testing.ts";

// testing.setFilter must be run before any tests are defined.
testing.setFilter(deno.argv[1]);

interface DenoPermissions {
  write?: boolean;
  net?: boolean;
}

function permToString(perms: DenoPermissions): string {
  const w = perms.write ? 1 : 0;
  const n = perms.net ? 1 : 0;
  return `permW${w}N${n}`;
}

function permFromString(s: string): DenoPermissions {
  const re = /^permW([01])N([01])$/;
  const found = s.match(re);
  if (!found) {
    throw Error("Not a permission string");
  }
  return {
    write: Boolean(Number(found[1])),
    net: Boolean(Number(found[2]))
  };
}

export function testPerm(perms: DenoPermissions, fn: testing.TestFunction) {
  const name = `${fn.name}_${permToString(perms)}`;
  testing.test({ fn, name });
}

export function test(fn: testing.TestFunction) {
  testPerm({ write: false, net: false }, fn);
}

test(function permSerialization() {
  for (let write of [true, false]) {
    for (let net of [true, false]) {
      let perms: DenoPermissions = { write, net };
      testing.assertEqual(perms, permFromString(permToString(perms)));
    }
  }
});

// To better catch internal errors, permFromString should throw if it gets an
// invalid permission string.
test(function permFromStringThrows() {
  let threw = false;
  try {
    permFromString("bad");
  } catch (e) {
    threw = true;
  }
  testing.assert(threw);
});
