// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
} from "../../../../test_util/std/testing/asserts.ts";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  link,
  linkSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { deferred } from "../../../../test_util/std/async/deferred.ts";

Deno.test(
  "[node/fs linkSync] link  creates newpath as a hard link to oldpath",
  () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const oldData = "Hardlink";
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    writeFileSync(oldName, oldData);
    // Create the hard link
    linkSync(oldName, newName);
    // We should expect reading the same content.
    const newData = readFileSync(newName, { encoding: "utf-8" });
    assertEquals(oldData, newData);
    // Writing to newname also affects oldname.
    const newData2 = "Modified";
    writeFileSync(newName, newData2);
    assertEquals(newData2, readFileSync(oldName, { encoding: "utf-8" }));
    // Writing to oldname also affects newname.
    const newData3 = "ModifiedAgain";
    writeFileSync(oldName, newData3);
    assertEquals(newData3, readFileSync(newName, { encoding: "utf-8" }));
    // Remove oldname. File still accessible through newname.
    rmSync(oldName);
    const newNameStat = statSync(newName);
    assert(newNameStat.isFile);
    assertEquals(newData3, readFileSync(newName, { encoding: "utf-8" }));
  },
);

Deno.test(
  "[node/fs linkSync] link throws when a file is already created with a new name.",
  () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    writeFileSync(oldName, "oldName");
    // newname is already created.
    writeFileSync(newName, "newName");

    assertThrows(
      () => {
        linkSync(oldName, newName);
      },
      Deno.errors.AlreadyExists,
      `link '${oldName}' -> '${newName}'`,
    );
  },
);

Deno.test(
  "[node/fs linkSync] link throws when file not found",
  () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";

    assertThrows(
      () => {
        linkSync(oldName, newName);
      },
      Deno.errors.NotFound,
      `link '${oldName}' -> '${newName}'`,
    );
  },
);

Deno.test(
  "[node/fs link] link  creates newpath as a hard link to oldpath",
  async () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const oldData = "Hardlink";
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    const d = deferred();
    writeFileSync(oldName, oldData);
    // Create the hard link
    link(oldName, newName, () => {
      d.resolve();
    });
    await d;
    // We should expect reading the same content.
    const newData = readFileSync(newName, { encoding: "utf-8" });
    assertEquals(oldData, newData);
    // Writing to newname also affects oldname.
    const newData2 = "Modified";
    writeFileSync(newName, newData2);
    assertEquals(newData2, readFileSync(oldName, { encoding: "utf-8" }));
    // Writing to oldname also affects newname.
    const newData3 = "ModifiedAgain";
    writeFileSync(oldName, newData3);
    assertEquals(newData3, readFileSync(newName, { encoding: "utf-8" }));
    // Remove oldname. File still accessible through newname.
    rmSync(oldName);
    const newNameStat = statSync(newName);
    assert(newNameStat.isFile);
    assertEquals(newData3, readFileSync(newName, { encoding: "utf-8" }));
  },
);

Deno.test(
  "[node/fs link] link throws when a file is already created with a new name.",
  () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";
    writeFileSync(oldName, "oldName");
    // newname is already created.
    writeFileSync(newName, "newName");

    const expectedMessageError = Deno.build.os == "windows"
      ? `Cannot create a file when that file already exists. (os error 183), link '${oldName}' -> '${newName}'`
      : `File exists (os error 17), link '${oldName}' -> '${newName}'`;
    link(oldName, newName, (err) => {
      assertEquals(err?.message, expectedMessageError);
    });
  },
);

Deno.test(
  "[node/fs link] link throws when file not found",
  () => {
    const testDir = mkdtempSync(join(tmpdir(), "foo-"));
    const oldName = testDir + "/oldname";
    const newName = testDir + "/newname";

    const expectedMessageError = Deno.build.os == "windows"
      ? `The system cannot find the file specified. (os error 17), link '${oldName}' -> '${newName}'`
      : `No such file or directory (os error 2), link '${oldName}' -> '${newName}'`;
    link(oldName, newName, (err) => {
      assertEquals(err?.message, expectedMessageError);
    });
  },
);
