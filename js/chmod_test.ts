// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, function chmodSyncSuccess() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  deno.writeFileSync(filename, data, 0o666);
  deno.chmodSync(filename, 0o777);
  const fileInfo = deno.statSync(filename);
  console.log(fileInfo.mode);
  assertEqual(fileInfo.mode, 0o777 );
});

testPerm({ write: false }, function chmodSyncPerm() {
  let err;
  try {
    const filename = deno.makeTempDirSync() + "/test.txt";
    deno.chmodSync(filename, 0o777);
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});
