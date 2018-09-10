// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, function mkdirSyncSuccess() {
  const path = deno.makeTempDirSync() + "/dir/subdir";
  deno.mkdirSync(path);
  const pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory());
});

testPerm({ write: false }, function mkdirSyncPerm() {
  let err;
  try {
    deno.mkdirSync("/baddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ write: true }, async function mkdirSuccess() {
  const path = deno.makeTempDirSync() + "/dir/subdir";
  await deno.mkdir(path);
  const pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory());
});
