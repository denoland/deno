// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, function renameSyncSuccess() {
  const testDir = deno.makeTempDirSync() + "/test-rename-sync";
  const oldpath = testDir + "/oldpath";
  const newpath = testDir + "/newpath";
  deno.mkdirSync(oldpath);
  deno.renameSync(oldpath, newpath);
  const newPathInfo = deno.statSync(newpath);
  assert(newPathInfo.isDirectory());

  let caughtErr = false;
  let oldPathInfo;

  try {
    oldPathInfo = deno.statSync(oldpath);
  } catch (e) {
    caughtErr = true;
    assertEqual(e.kind, deno.ErrorKind.NotFound);
  }
  assert(caughtErr);
  assertEqual(oldPathInfo, undefined);
});

testPerm({ write: false }, function renameSyncPerm() {
  let err;
  try {
    const oldpath = "/oldbaddir";
    const newpath = "/newbaddir";
    deno.renameSync(oldpath, newpath);
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ write: true }, async function renameSuccess() {
  const testDir = deno.makeTempDirSync() + "/test-rename";
  const oldpath = testDir + "/oldpath";
  const newpath = testDir + "/newpath";
  deno.mkdirSync(oldpath);
  await deno.rename(oldpath, newpath);
  const newPathInfo = deno.statSync(newpath);
  assert(newPathInfo.isDirectory());

  let caughtErr = false;
  let oldPathInfo;

  try {
    oldPathInfo = deno.statSync(oldpath);
  } catch (e) {
    caughtErr = true;
    assertEqual(e.kind, deno.ErrorKind.NotFound);
  }
  assert(caughtErr);
  assertEqual(oldPathInfo, undefined);
});
