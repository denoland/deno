// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
} from "./test_util.ts";

Deno.test(
  { permissions: { read: true, write: true } },
  function linkSyncSuccess() {
    const testDir = Deno.makeTempDirSync();
    const oldData = "Hardlink";
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    Deno.writeFileSync(oldName, new TextEncoder().encode(oldData));
    // Create the hard link.
    Deno.linkSync(oldName, newName);
    // We should expect reading the same content.
    const newData = Deno.readTextFileSync(newName);
    assertEquals(oldData, newData);
    // Writing to newname also affects oldname.
    const newData2 = "Modified";
    Deno.writeFileSync(newName, new TextEncoder().encode(newData2));
    assertEquals(
      newData2,
      Deno.readTextFileSync(oldName),
    );
    // Writing to oldname also affects newname.
    const newData3 = "ModifiedAgain";
    Deno.writeFileSync(oldName, new TextEncoder().encode(newData3));
    assertEquals(
      newData3,
      Deno.readTextFileSync(newName),
    );
    // Remove oldname. File still accessible through newname.
    Deno.removeSync(oldName);
    const newNameStat = Deno.statSync(newName);
    assert(newNameStat.isFile);
    assert(!newNameStat.isSymlink); // Not a symlink.
    assertEquals(
      newData3,
      Deno.readTextFileSync(newName),
    );
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function linkSyncExists() {
    const testDir = Deno.makeTempDirSync();
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    Deno.writeFileSync(oldName, new TextEncoder().encode("oldName"));
    // newname is already created.
    Deno.writeFileSync(newName, new TextEncoder().encode("newName"));

    assertThrows(
      () => {
        Deno.linkSync(oldName, newName);
      },
      Deno.errors.AlreadyExists,
      `link '${oldName}' -> '${newName}'`,
    );
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function linkSyncNotFound() {
    const testDir = Deno.makeTempDirSync();
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";

    assertThrows(
      () => {
        Deno.linkSync(oldName, newName);
      },
      Deno.errors.NotFound,
      `link '${oldName}' -> '${newName}'`,
    );
  },
);

Deno.test(
  { permissions: { read: false, write: true } },
  function linkSyncReadPerm() {
    assertThrows(() => {
      Deno.linkSync("oldbaddir", "newbaddir");
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: false } },
  function linkSyncWritePerm() {
    assertThrows(() => {
      Deno.linkSync("oldbaddir", "newbaddir");
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function linkSuccess() {
    const testDir = Deno.makeTempDirSync();
    const oldData = "Hardlink";
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    Deno.writeFileSync(oldName, new TextEncoder().encode(oldData));
    // Create the hard link.
    await Deno.link(oldName, newName);
    // We should expect reading the same content.
    const newData = Deno.readTextFileSync(newName);
    assertEquals(oldData, newData);
    // Writing to newname also affects oldname.
    const newData2 = "Modified";
    Deno.writeFileSync(newName, new TextEncoder().encode(newData2));
    assertEquals(
      newData2,
      Deno.readTextFileSync(oldName),
    );
    // Writing to oldname also affects newname.
    const newData3 = "ModifiedAgain";
    Deno.writeFileSync(oldName, new TextEncoder().encode(newData3));
    assertEquals(
      newData3,
      Deno.readTextFileSync(newName),
    );
    // Remove oldname. File still accessible through newname.
    Deno.removeSync(oldName);
    const newNameStat = Deno.statSync(newName);
    assert(newNameStat.isFile);
    assert(!newNameStat.isSymlink); // Not a symlink.
    assertEquals(
      newData3,
      Deno.readTextFileSync(newName),
    );
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function linkExists() {
    const testDir = Deno.makeTempDirSync();
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    Deno.writeFileSync(oldName, new TextEncoder().encode("oldName"));
    // newname is already created.
    Deno.writeFileSync(newName, new TextEncoder().encode("newName"));

    await assertRejects(
      async () => {
        await Deno.link(oldName, newName);
      },
      Deno.errors.AlreadyExists,
      `link '${oldName}' -> '${newName}'`,
    );
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function linkNotFound() {
    const testDir = Deno.makeTempDirSync();
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";

    await assertRejects(
      async () => {
        await Deno.link(oldName, newName);
      },
      Deno.errors.NotFound,
      `link '${oldName}' -> '${newName}'`,
    );
  },
);

Deno.test(
  { permissions: { read: false, write: true } },
  async function linkReadPerm() {
    await assertRejects(async () => {
      await Deno.link("oldbaddir", "newbaddir");
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: false } },
  async function linkWritePerm() {
    await assertRejects(async () => {
      await Deno.link("oldbaddir", "newbaddir");
    }, Deno.errors.NotCapable);
  },
);
