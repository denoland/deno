// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function linkSyncSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const oldData = "Hardlink";
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    Deno.writeFileSync(oldName, new TextEncoder().encode(oldData));
    // Create the hard link.
    Deno.linkSync(oldName, newName);
    // We should expect reading the same content.
    const newData = new TextDecoder().decode(Deno.readFileSync(newName));
    assertEquals(oldData, newData);
    // Writing to newname also affects oldname.
    const newData2 = "Modified";
    Deno.writeFileSync(newName, new TextEncoder().encode(newData2));
    assertEquals(
      newData2,
      new TextDecoder().decode(Deno.readFileSync(oldName))
    );
    // Writing to oldname also affects newname.
    const newData3 = "ModifiedAgain";
    Deno.writeFileSync(oldName, new TextEncoder().encode(newData3));
    assertEquals(
      newData3,
      new TextDecoder().decode(Deno.readFileSync(newName))
    );
    // Remove oldname. File still accessible through newname.
    Deno.removeSync(oldName);
    const newNameStat = Deno.statSync(newName);
    assert(newNameStat.isFile());
    assert(!newNameStat.isSymlink()); // Not a symlink.
    assertEquals(
      newData3,
      new TextDecoder().decode(Deno.readFileSync(newName))
    );
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function linkSyncExists(): void {
    const testDir = Deno.makeTempDirSync();
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    Deno.writeFileSync(oldName, new TextEncoder().encode("oldName"));
    // newname is already created.
    Deno.writeFileSync(newName, new TextEncoder().encode("newName"));

    let err;
    try {
      Deno.linkSync(oldName, newName);
    } catch (e) {
      err = e;
    }
    assert(!!err);
    assert(err instanceof Deno.errors.AlreadyExists);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function linkSyncNotFound(): void {
    const testDir = Deno.makeTempDirSync();
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";

    let err;
    try {
      Deno.linkSync(oldName, newName);
    } catch (e) {
      err = e;
    }
    assert(!!err);
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { read: false, write: true } },
  function linkSyncReadPerm(): void {
    let err;
    try {
      Deno.linkSync("oldbaddir", "newbaddir");
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
);

unitTest(
  { perms: { read: true, write: false } },
  function linkSyncWritePerm(): void {
    let err;
    try {
      Deno.linkSync("oldbaddir", "newbaddir");
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function linkSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const oldData = "Hardlink";
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    Deno.writeFileSync(oldName, new TextEncoder().encode(oldData));
    // Create the hard link.
    await Deno.link(oldName, newName);
    // We should expect reading the same content.
    const newData = new TextDecoder().decode(Deno.readFileSync(newName));
    assertEquals(oldData, newData);
    // Writing to newname also affects oldname.
    const newData2 = "Modified";
    Deno.writeFileSync(newName, new TextEncoder().encode(newData2));
    assertEquals(
      newData2,
      new TextDecoder().decode(Deno.readFileSync(oldName))
    );
    // Writing to oldname also affects newname.
    const newData3 = "ModifiedAgain";
    Deno.writeFileSync(oldName, new TextEncoder().encode(newData3));
    assertEquals(
      newData3,
      new TextDecoder().decode(Deno.readFileSync(newName))
    );
    // Remove oldname. File still accessible through newname.
    Deno.removeSync(oldName);
    const newNameStat = Deno.statSync(newName);
    assert(newNameStat.isFile());
    assert(!newNameStat.isSymlink()); // Not a symlink.
    assertEquals(
      newData3,
      new TextDecoder().decode(Deno.readFileSync(newName))
    );
  }
);
