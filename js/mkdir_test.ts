// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, function mkdirSyncSuccess() {
  const path = deno.makeTempDirSync() + "/dir/subdir";
  deno.mkdirSync(path);
  const pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory());
});

testPerm({ write: true }, function mkdirSyncMode() {
  const path = deno.makeTempDirSync() + "/dir/subdir";
  deno.mkdirSync(path, 0o755); // no perm for x
  const pathInfo = deno.statSync(path);
  if (pathInfo.mode !== null) {
    // Skip windows
    assertEqual(pathInfo.mode & 0o777, 0o755);
  }
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
