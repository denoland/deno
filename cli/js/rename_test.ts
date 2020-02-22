// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

testPerm({ read: true, write: true }, function renameSyncSuccess(): void {
  const testDir = Deno.makeTempDirSync();
  const oldpath = testDir + "/oldpath";
  const newpath = testDir + "/newpath";
  Deno.mkdirSync(oldpath);
  Deno.renameSync(oldpath, newpath);
  const newPathInfo = Deno.statSync(newpath);
  assert(newPathInfo.isDirectory());

  let caughtErr = false;
  let oldPathInfo;

  try {
    oldPathInfo = Deno.statSync(oldpath);
  } catch (e) {
    caughtErr = true;
    assert(e instanceof Deno.Err.NotFound);
  }
  assert(caughtErr);
  assertEquals(oldPathInfo, undefined);
});

testPerm({ read: false, write: true }, function renameSyncReadPerm(): void {
  let err;
  try {
    const oldpath = "/oldbaddir";
    const newpath = "/newbaddir";
    Deno.renameSync(oldpath, newpath);
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.Err.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: false }, function renameSyncWritePerm(): void {
  let err;
  try {
    const oldpath = "/oldbaddir";
    const newpath = "/newbaddir";
    Deno.renameSync(oldpath, newpath);
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.Err.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: true }, async function renameSuccess(): Promise<
  void
> {
  const testDir = Deno.makeTempDirSync();
  const oldpath = testDir + "/oldpath";
  const newpath = testDir + "/newpath";
  Deno.mkdirSync(oldpath);
  await Deno.rename(oldpath, newpath);
  const newPathInfo = Deno.statSync(newpath);
  assert(newPathInfo.isDirectory());

  let caughtErr = false;
  let oldPathInfo;

  try {
    oldPathInfo = Deno.statSync(oldpath);
  } catch (e) {
    caughtErr = true;
    assert(e instanceof Deno.Err.NotFound);
  }
  assert(caughtErr);
  assertEquals(oldPathInfo, undefined);
});
