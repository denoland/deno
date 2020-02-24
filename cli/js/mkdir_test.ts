// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

testPerm({ read: true, write: true }, function mkdirSyncSuccess(): void {
  const path = Deno.makeTempDirSync() + "/dir";
  Deno.mkdirSync(path);
  const pathInfo = Deno.statSync(path);
  assert(pathInfo.isDirectory());
});

testPerm({ read: true, write: true }, function mkdirSyncMode(): void {
  const path = Deno.makeTempDirSync() + "/dir";
  Deno.mkdirSync(path, { mode: 0o755 }); // no perm for x
  const pathInfo = Deno.statSync(path);
  if (pathInfo.mode !== null) {
    // Skip windows
    assertEquals(pathInfo.mode & 0o777, 0o755);
  }
});

testPerm({ write: false }, function mkdirSyncPerm(): void {
  let err;
  try {
    Deno.mkdirSync("/baddir");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: true }, async function mkdirSuccess(): Promise<
  void
> {
  const path = Deno.makeTempDirSync() + "/dir";
  await Deno.mkdir(path);
  const pathInfo = Deno.statSync(path);
  assert(pathInfo.isDirectory());
});

testPerm({ write: true }, function mkdirErrIfExists(): void {
  let err;
  try {
    Deno.mkdirSync(".");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.AlreadyExists);
});

testPerm({ read: true, write: true }, function mkdirSyncRecursive(): void {
  const path = Deno.makeTempDirSync() + "/nested/directory";
  Deno.mkdirSync(path, { recursive: true });
  const pathInfo = Deno.statSync(path);
  assert(pathInfo.isDirectory());
});

testPerm({ read: true, write: true }, async function mkdirRecursive(): Promise<
  void
> {
  const path = Deno.makeTempDirSync() + "/nested/directory";
  await Deno.mkdir(path, { recursive: true });
  const pathInfo = Deno.statSync(path);
  assert(pathInfo.isDirectory());
});
