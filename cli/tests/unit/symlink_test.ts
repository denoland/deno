// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertThrows } from "./test_util.ts";

Deno.test("symlinkSyncSuccess", function (): void {
  const testDir = Deno.makeTempDirSync();
  const oldname = testDir + "/oldname";
  const newname = testDir + "/newname";
  Deno.mkdirSync(oldname);
  Deno.symlinkSync(oldname, newname);
  const newNameInfoLStat = Deno.lstatSync(newname);
  const newNameInfoStat = Deno.statSync(newname);
  assert(newNameInfoLStat.isSymlink);
  assert(newNameInfoStat.isDirectory);
});

Deno.test("symlinkSuccess", async function (): Promise<void> {
  const testDir = Deno.makeTempDirSync();
  const oldname = testDir + "/oldname";
  const newname = testDir + "/newname";
  Deno.mkdirSync(oldname);
  await Deno.symlink(oldname, newname);
  const newNameInfoLStat = Deno.lstatSync(newname);
  const newNameInfoStat = Deno.statSync(newname);
  assert(newNameInfoLStat.isSymlink, "NOT SYMLINK");
  assert(newNameInfoStat.isDirectory, "NOT DIRECTORY");
});

Deno.test("symlinkSyncPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "write" });

  assertThrows(() => {
    Deno.symlinkSync("oldbaddir", "newbaddir");
  }, Deno.errors.PermissionDenied);
});
