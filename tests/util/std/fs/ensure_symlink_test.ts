// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// TODO(axetroy): Add test for Windows once symlink is implemented for Windows.
import { assertEquals, assertRejects, assertThrows } from "../assert/mod.ts";
import * as path from "../path/mod.ts";
import { ensureSymlink, ensureSymlinkSync } from "./ensure_symlink.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testdataDir = path.resolve(moduleDir, "testdata");

Deno.test("ensureSymlink() rejects if file does not exist", async function () {
  const testDir = path.join(testdataDir, "link_file_1");
  const testFile = path.join(testDir, "test.txt");

  await assertRejects(
    async () => {
      await ensureSymlink(testFile, path.join(testDir, "test1.txt"));
    },
  );

  await assertRejects(
    async () => {
      await Deno.stat(testFile).then(() => {
        throw new Error("test file should exist.");
      });
    },
  );
});

Deno.test("ensureSymlinkSync() throws if file does not exist", function () {
  const testDir = path.join(testdataDir, "link_file_2");
  const testFile = path.join(testDir, "test.txt");

  assertThrows(() => {
    ensureSymlinkSync(testFile, path.join(testDir, "test1.txt"));
  });

  assertThrows(() => {
    Deno.statSync(testFile);
    throw new Error("test file should exist.");
  });
});

Deno.test("ensureSymlink() ensures linkName links to target", async function () {
  const testDir = path.join(testdataDir, "link_file_3");
  const testFile = path.join(testDir, "test.txt");
  const linkFile = path.join(testDir, "link.txt");

  await Deno.mkdir(testDir, { recursive: true });
  await Deno.writeFile(testFile, new Uint8Array());

  await ensureSymlink(testFile, linkFile);
  await ensureSymlink(testFile, linkFile);

  const srcStat = await Deno.lstat(testFile);
  const linkStat = await Deno.lstat(linkFile);

  assertEquals(srcStat.isFile, true);
  assertEquals(linkStat.isSymlink, true);

  await Deno.remove(testDir, { recursive: true });
});

Deno.test("ensureSymlinkSync() ensures linkName links to target", function () {
  const testDir = path.join(testdataDir, "link_file_4");
  const testFile = path.join(testDir, "test.txt");
  const linkFile = path.join(testDir, "link.txt");

  Deno.mkdirSync(testDir, { recursive: true });
  Deno.writeFileSync(testFile, new Uint8Array());

  ensureSymlinkSync(testFile, linkFile);
  ensureSymlinkSync(testFile, linkFile);

  const srcStat = Deno.lstatSync(testFile);
  const linkStat = Deno.lstatSync(linkFile);

  assertEquals(srcStat.isFile, true);
  assertEquals(linkStat.isSymlink, true);

  Deno.removeSync(testDir, { recursive: true });
});

Deno.test("ensureSymlink() ensures dir linkName links to dir target", async function () {
  const testDir = path.join(testdataDir, "link_file_origin_3");
  const linkDir = path.join(testdataDir, "link_file_link_3");
  const testFile = path.join(testDir, "test.txt");

  await Deno.mkdir(testDir, { recursive: true });
  await Deno.writeFile(testFile, new Uint8Array());

  await ensureSymlink(testDir, linkDir);
  await ensureSymlink(testDir, linkDir);

  const testDirStat = await Deno.lstat(testDir);
  const linkDirStat = await Deno.lstat(linkDir);
  const testFileStat = await Deno.lstat(testFile);

  assertEquals(testFileStat.isFile, true);
  assertEquals(testDirStat.isDirectory, true);
  assertEquals(linkDirStat.isSymlink, true);

  await Deno.remove(linkDir, { recursive: true });
  await Deno.remove(testDir, { recursive: true });
});

Deno.test("ensureSymlinkSync() ensures dir linkName links to dir target", function () {
  const testDir = path.join(testdataDir, "link_file_origin_3");
  const linkDir = path.join(testdataDir, "link_file_link_3");
  const testFile = path.join(testDir, "test.txt");

  Deno.mkdirSync(testDir, { recursive: true });
  Deno.writeFileSync(testFile, new Uint8Array());

  ensureSymlinkSync(testDir, linkDir);
  ensureSymlinkSync(testDir, linkDir);

  const testDirStat = Deno.lstatSync(testDir);
  const linkDirStat = Deno.lstatSync(linkDir);
  const testFileStat = Deno.lstatSync(testFile);

  assertEquals(testFileStat.isFile, true);
  assertEquals(testDirStat.isDirectory, true);
  assertEquals(linkDirStat.isSymlink, true);

  Deno.removeSync(linkDir, { recursive: true });
  Deno.removeSync(testDir, { recursive: true });
});

Deno.test("ensureSymlink() creates symlink with relative target", async function () {
  const testDir = path.join(testdataDir, "symlink-relative");
  const testLinkName = path.join(testDir, "link.txt");
  const testFile = path.join(testDir, "target.txt");

  await Deno.mkdir(testDir);

  await Deno.writeFile(testFile, new Uint8Array());

  await ensureSymlink("target.txt", testLinkName);

  const testDirStat = await Deno.lstat(testDir);
  const linkDirStat = await Deno.lstat(testLinkName);
  const testFileStat = await Deno.lstat(testFile);

  assertEquals(testFileStat.isFile, true);
  assertEquals(testDirStat.isDirectory, true);
  assertEquals(linkDirStat.isSymlink, true);

  await Deno.remove(testDir, { recursive: true });
});

Deno.test("ensureSymlinkSync() creates symlink with relative target", function () {
  const testDir = path.join(testdataDir, "symlink-relative-sync");
  const testLinkName = path.join(testDir, "link.txt");
  const testFile = path.join(testDir, "target.txt");

  Deno.mkdirSync(testDir);

  Deno.writeFileSync(testFile, new Uint8Array());

  ensureSymlinkSync("target.txt", testLinkName);

  const testDirStat = Deno.lstatSync(testDir);
  const linkDirStat = Deno.lstatSync(testLinkName);
  const testFileStat = Deno.lstatSync(testFile);

  assertEquals(testFileStat.isFile, true);
  assertEquals(testDirStat.isDirectory, true);
  assertEquals(linkDirStat.isSymlink, true);

  Deno.removeSync(testDir, { recursive: true });
});
