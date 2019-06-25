// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import {
  test,
  testPerm,
  assert,
  assertEquals,
  assertNotEquals
} from "./test_util.ts";

testPerm({ env: true }, function envSuccess(): void {
  const env = Deno.env();
  assert(env !== null);
  // eslint-disable-next-line @typescript-eslint/camelcase
  env.test_var = "Hello World";
  const newEnv = Deno.env();
  assertEquals(env.test_var, newEnv.test_var);
});

test(function envFailure(): void {
  let caughtError = false;
  try {
    Deno.env();
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }

  assert(caughtError);
});

test(function osPid(): void {
  console.log("pid", Deno.pid);
  assert(Deno.pid > 0);
});

// See complete tests in tools/is_tty_test.py
test(function osIsTTYSmoke(): void {
  console.log(Deno.isTTY());
});

test(function homeDir(): void {
  assertNotEquals(Deno.homeDir(), "");
});
