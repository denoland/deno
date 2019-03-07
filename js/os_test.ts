// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEquals } from "./test_util.ts";

testPerm({ env: true }, function envSuccess() {
  const env = Deno.env();
  assert(env !== null);
  env.test_var = "Hello World";
  const newEnv = Deno.env();
  assertEquals(env.test_var, newEnv.test_var);
});

test(function envFailure() {
  let caughtError = false;
  try {
    const env = Deno.env();
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }

  assert(caughtError);
});

test(function osPid() {
  console.log("pid", Deno.pid);
  assert(Deno.pid > 0);
});

// See complete tests in tools/is_tty_test.py
test(function osIsTTYSmoke() {
  console.log(Deno.isTTY());
});
