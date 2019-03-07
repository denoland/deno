// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

testPerm({ read: true, write: true }, function mkdirSyncSuccess() {
  const path = Deno.makeTempDirSync() + "/dir";
  Deno.mkdirSync(path);
  const pathInfo = Deno.statSync(path);
  assert(pathInfo.isDirectory());
});

testPerm({ read: true, write: true }, function mkdirSyncMode() {
  const path = Deno.makeTempDirSync() + "/dir";
  Deno.mkdirSync(path, false, 0o755); // no perm for x
  const pathInfo = Deno.statSync(path);
  if (pathInfo.mode !== null) {
    // Skip windows
    assertEquals(pathInfo.mode & 0o777, 0o755);
  }
});

testPerm({ write: false }, function mkdirSyncPerm() {
  let err;
  try {
    Deno.mkdirSync("/baddir");
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: true }, async function mkdirSuccess() {
  const path = Deno.makeTempDirSync() + "/dir";
  await Deno.mkdir(path);
  const pathInfo = Deno.statSync(path);
  assert(pathInfo.isDirectory());
});

testPerm({ write: true }, function mkdirErrIfExists() {
  let err;
  try {
    Deno.mkdirSync(".");
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.AlreadyExists);
  assertEquals(err.name, "AlreadyExists");
});

testPerm({ read: true, write: true }, function mkdirSyncRecursive() {
  const path = Deno.makeTempDirSync() + "/nested/directory";
  Deno.mkdirSync(path, true);
  const pathInfo = Deno.statSync(path);
  assert(pathInfo.isDirectory());
});

testPerm({ read: true, write: true }, async function mkdirRecursive() {
  const path = Deno.makeTempDirSync() + "/nested/directory";
  await Deno.mkdir(path, true);
  const pathInfo = Deno.statSync(path);
  assert(pathInfo.isDirectory());
});
