// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertThrows, unitTest } from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function symlinkSyncSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    Deno.symlinkSync(oldname, newname);
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink);
    assert(newNameInfoStat.isDirectory);
  },
);

unitTest(function symlinkSyncPerm(): void {
  assertThrows(() => {
    Deno.symlinkSync("oldbaddir", "newbaddir");
  }, Deno.errors.PermissionDenied);
});

unitTest(
  { perms: { read: true, write: true } },
  async function symlinkSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    await Deno.symlink(oldname, newname);
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink, "NOT SYMLINK");
    assert(newNameInfoStat.isDirectory, "NOT DIRECTORY");
  },
);
