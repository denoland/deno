// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";

testPerm({ read: true, write: true }, function renameSyncSuccess() {
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
    assertEqual(e.kind, Deno.ErrorKind.NotFound);
  }
  assert(caughtErr);
  assertEqual(oldPathInfo, undefined);
});

testPerm({ read: true, write: false }, function renameSyncPerm() {
  let err;
  try {
    const oldpath = "/oldbaddir";
    const newpath = "/newbaddir";
    Deno.renameSync(oldpath, newpath);
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ read: true, write: true }, async function renameSuccess() {
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
    assertEqual(e.kind, Deno.ErrorKind.NotFound);
  }
  assert(caughtErr);
  assertEqual(oldPathInfo, undefined);
});
