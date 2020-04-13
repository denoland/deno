// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function symlinkSyncSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    let errOnWindows;
    // Just for now, until we implement symlink for Windows.
    try {
      Deno.symlinkSync(oldname, newname);
    } catch (e) {
      errOnWindows = e;
    }
    if (errOnWindows) {
      assertEquals(Deno.build.os, "win");
      assertEquals(errOnWindows.message, "not implemented");
    } else {
      const newNameInfoLStat = Deno.lstatSync(newname);
      const newNameInfoStat = Deno.statSync(newname);
      assert(newNameInfoLStat.isSymlink());
      assert(newNameInfoStat.isDirectory());
    }
  }
);

unitTest(function symlinkSyncPerm(): void {
  let err;
  try {
    Deno.symlinkSync("oldbaddir", "newbaddir");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

// Just for now, until we implement symlink for Windows.
// Symlink with type should succeed on other platforms with type ignored
unitTest(
  { perms: { write: true } },
  function symlinkSyncNotImplemented(): void {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    let err;
    try {
      Deno.symlinkSync(oldname, newname, "dir");
    } catch (e) {
      err = e;
    }
    if (err) {
      assertEquals(Deno.build.os, "win");
      // from cli/js/util.ts:notImplemented
      assertEquals(err.message, "not implemented");
    }
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function symlinkSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    let errOnWindows;
    // Just for now, until we implement symlink for Windows.
    try {
      await Deno.symlink(oldname, newname);
    } catch (e) {
      errOnWindows = e;
    }
    if (errOnWindows) {
      assertEquals(errOnWindows.message, "not implemented");
    } else {
      const newNameInfoLStat = Deno.lstatSync(newname);
      const newNameInfoStat = Deno.statSync(newname);
      assert(newNameInfoLStat.isSymlink());
      assert(newNameInfoStat.isDirectory());
    }
  }
);
