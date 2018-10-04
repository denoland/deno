// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, function readlinkSyncSuccess() {
  const testDir = deno.makeTempDirSync() + "/test-readlink-sync";
  const target = testDir + "/target";
  const symlink = testDir + "/symln";
  deno.mkdirSync(target);
  // TODO Add test for Windows once symlink is implemented for Windows.
  // See https://github.com/denoland/deno/issues/815.
  if (deno.platform.os !== "win") {
    deno.symlinkSync(target, symlink);
    const targetPath = deno.readlinkSync(symlink);
    assertEqual(targetPath, target);
  }
});

test(function readlinkSyncNotFound() {
  let caughtError = false;
  let data;
  try {
    data = deno.readlinkSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.NotFound);
  }
  assert(caughtError);
  assertEqual(data, undefined);
});

testPerm({ write: true }, async function readlinkSuccess() {
  const testDir = deno.makeTempDirSync() + "/test-readlink";
  const target = testDir + "/target";
  const symlink = testDir + "/symln";
  deno.mkdirSync(target);
  // TODO Add test for Windows once symlink is implemented for Windows.
  // See https://github.com/denoland/deno/issues/815.
  if (deno.platform.os !== "win") {
    deno.symlinkSync(target, symlink);
    const targetPath = await deno.readlink(symlink);
    assertEqual(targetPath, target);
  }
});
