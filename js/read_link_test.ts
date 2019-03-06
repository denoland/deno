// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";

testPerm({ write: true, read: true }, function readlinkSyncSuccess() {
  const testDir = Deno.makeTempDirSync();
  const target = testDir + "/target";
  const symlink = testDir + "/symln";
  Deno.mkdirSync(target);
  // TODO Add test for Windows once symlink is implemented for Windows.
  // See https://github.com/denoland/deno/issues/815.
  if (Deno.build.os !== "win") {
    Deno.symlinkSync(target, symlink);
    const targetPath = Deno.readlinkSync(symlink);
    assertEqual(targetPath, target);
  }
});

testPerm({ read: false }, async function readlinkSyncPerm() {
  let caughtError = false;
  try {
    Deno.readlinkSync("/symlink");
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true }, function readlinkSyncNotFound() {
  let caughtError = false;
  let data;
  try {
    data = Deno.readlinkSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.NotFound);
  }
  assert(caughtError);
  assertEqual(data, undefined);
});

testPerm({ write: true, read: true }, async function readlinkSuccess() {
  const testDir = Deno.makeTempDirSync();
  const target = testDir + "/target";
  const symlink = testDir + "/symln";
  Deno.mkdirSync(target);
  // TODO Add test for Windows once symlink is implemented for Windows.
  // See https://github.com/denoland/deno/issues/815.
  if (Deno.build.os !== "win") {
    Deno.symlinkSync(target, symlink);
    const targetPath = await Deno.readlink(symlink);
    assertEqual(targetPath, target);
  }
});

testPerm({ read: false }, async function readlinkPerm() {
  let caughtError = false;
  try {
    await Deno.readlink("/symlink");
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});
