// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, function symlinkSyncSuccess() {
  const testDir = deno.makeTempDirSync() + "/test-symlink-sync";
  const oldname = testDir + "/oldname";
  const newname = testDir + "/newname";
  deno.mkdirSync(oldname);
  deno.symlinkSync(oldname, newname);
  const newNameInfoLStat = deno.lstatSync(newname);
  const newNameInfoStat = deno.statSync(newname);
  assert(newNameInfoLStat.isSymlink());
  assert(newNameInfoStat.isDirectory());
});

testPerm({ write: false }, function symlinkSyncPerm() {
  let err;
  try {
    deno.symlinkSync("oldbaddir", "newbaddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ write: true }, async function symlinkSuccess() {
  const testDir = deno.makeTempDirSync() + "/test-symlink";
  const oldname = testDir + "/oldname";
  const newname = testDir + "/newname";
  deno.mkdirSync(oldname);
  await deno.symlink(oldname, newname);
  const newNameInfoLStat = deno.lstatSync(newname);
  const newNameInfoStat = deno.statSync(newname);
  assert(newNameInfoLStat.isSymlink());
  assert(newNameInfoStat.isDirectory());
});
