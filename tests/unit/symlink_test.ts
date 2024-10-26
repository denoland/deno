// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

Deno.test(
  { permissions: { read: true, write: true } },
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

Deno.test(
  { permissions: { read: true, write: true } },
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

Deno.test(
  {
    ignore: Deno.build.os !== "windows",
    permissions: { read: true, write: true },
  },
  function symlinkSyncJunction() {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    Deno.symlinkSync(oldname, newname, { type: "junction" });
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink);
    assert(newNameInfoStat.isDirectory);
  },
);

Deno.test(
  { permissions: { read: false, write: false } },
  function symlinkSyncPerm() {
    assertThrows(() => {
      Deno.symlinkSync("oldbaddir", "newbaddir");
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function symlinkSyncAlreadyExist() {
    const existingFile = Deno.makeTempFileSync();
    const existingFile2 = Deno.makeTempFileSync();
    assertThrows(
      () => {
        Deno.symlinkSync(existingFile, existingFile2);
      },
      Deno.errors.AlreadyExists,
      `symlink '${existingFile}' -> '${existingFile2}'`,
    );
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
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

Deno.test(
  { permissions: { read: true, write: true } },
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

Deno.test(
  {
    ignore: Deno.build.os !== "windows",
    permissions: { read: true, write: true },
  },
  async function symlinkJunction() {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    await Deno.symlink(oldname, newname, { type: "junction" });
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink, "NOT SYMLINK");
    assert(newNameInfoStat.isDirectory, "NOT DIRECTORY");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function symlinkAlreadyExist() {
    const existingFile = Deno.makeTempFileSync();
    const existingFile2 = Deno.makeTempFileSync();
    await assertRejects(
      async () => {
        await Deno.symlink(existingFile, existingFile2);
      },
      Deno.errors.AlreadyExists,
      `symlink '${existingFile}' -> '${existingFile2}'`,
    );
  },
);

Deno.test(
  { permissions: { read: true, write: ["."] } },
  async function symlinkNoFullWritePermissions() {
    await assertRejects(
      () => Deno.symlink("old", "new"),
      Deno.errors.NotCapable,
    );
    assertThrows(
      () => Deno.symlinkSync("old", "new"),
      Deno.errors.NotCapable,
    );
  },
);

Deno.test(
  { permissions: { read: ["."], write: true } },
  async function symlinkNoFullReadPermissions() {
    await assertRejects(
      () => Deno.symlink("old", "new"),
      Deno.errors.NotCapable,
    );
    assertThrows(
      () => Deno.symlinkSync("old", "new"),
      Deno.errors.NotCapable,
    );
  },
);
