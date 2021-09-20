// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertThrows,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function symlinkSyncSuccess() {
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

unitTest(
  { perms: { read: true, write: true } },
  function symlinkSyncURL() {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    Deno.symlinkSync(
      pathToAbsoluteFileUrl(oldname),
      pathToAbsoluteFileUrl(newname),
    );
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink);
    assert(newNameInfoStat.isDirectory);
  },
);

unitTest(function symlinkSyncPerm() {
  assertThrows(() => {
    Deno.symlinkSync("oldbaddir", "newbaddir");
  }, Deno.errors.PermissionDenied);
});

unitTest(
  { perms: { read: true, write: true } },
  async function symlinkSuccess() {
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

unitTest(
  { perms: { read: true, write: true } },
  async function symlinkURL() {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    await Deno.symlink(
      pathToAbsoluteFileUrl(oldname),
      pathToAbsoluteFileUrl(newname),
    );
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink, "NOT SYMLINK");
    assert(newNameInfoStat.isDirectory, "NOT DIRECTORY");
  },
);
