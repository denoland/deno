// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ env: true }, function envSuccess() {
  const env = deno.env();
  assert(env !== null);
  env.test_var = "Hello World";
  const newEnv = deno.env();
  assertEqual(env.test_var, newEnv.test_var);
});

test(function envFailure() {
  let caughtError = false;
  try {
    const env = deno.env();
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
    assertEqual(err.name, "PermissionDenied");
  }

  assert(caughtError);
});

test(function osPid() {
  console.log("pid", deno.pid);
  assert(deno.pid > 0);
});
