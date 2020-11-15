// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// TODO(axetroy): Add test for Windows once symlink is implemented for Windows.
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import { ensureSymlink, ensureSymlinkSync } from "./ensure_symlink.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testdataDir = path.resolve(Deno.cwd(), moduleDir, "testdata");

Deno.test("ensureSymlinkIfItNotExist", async function (): Promise<void> {
  const testDir = path.join(testdataDir, "link_file_1");
  const testFile = path.join(testDir, "test.txt");

  await assertThrowsAsync(
    async (): Promise<void> => {
      await ensureSymlink(testFile, path.join(testDir, "test1.txt"));
    },
  );

  await assertThrowsAsync(
    async (): Promise<void> => {
      await Deno.stat(testFile).then((): void => {
        throw new Error("test file should exists.");
      });
    },
  );
});

Deno.test("ensureSymlinkSyncIfItNotExist", function (): void {
  const testDir = path.join(testdataDir, "link_file_2");
  const testFile = path.join(testDir, "test.txt");

  assertThrows((): void => {
    ensureSymlinkSync(testFile, path.join(testDir, "test1.txt"));
  });

  assertThrows((): void => {
    Deno.statSync(testFile);
    throw new Error("test file should exists.");
  });
});

Deno.test("ensureSymlinkIfItExist", async function (): Promise<void> {
  const testDir = path.join(testdataDir, "link_file_3");
  const testFile = path.join(testDir, "test.txt");
  const linkFile = path.join(testDir, "link.txt");

  await Deno.mkdir(testDir, { recursive: true });
  await Deno.writeFile(testFile, new Uint8Array());

  await ensureSymlink(testFile, linkFile);

  const srcStat = await Deno.lstat(testFile);
  const linkStat = await Deno.lstat(linkFile);

  assertEquals(srcStat.isFile, true);
  assertEquals(linkStat.isSymlink, true);

  await Deno.remove(testDir, { recursive: true });
});

Deno.test("ensureSymlinkSyncIfItExist", function (): void {
  const testDir = path.join(testdataDir, "link_file_4");
  const testFile = path.join(testDir, "test.txt");
  const linkFile = path.join(testDir, "link.txt");

  Deno.mkdirSync(testDir, { recursive: true });
  Deno.writeFileSync(testFile, new Uint8Array());

  ensureSymlinkSync(testFile, linkFile);

  const srcStat = Deno.lstatSync(testFile);

  const linkStat = Deno.lstatSync(linkFile);

  assertEquals(srcStat.isFile, true);
  assertEquals(linkStat.isSymlink, true);

  Deno.removeSync(testDir, { recursive: true });
});

Deno.test("ensureSymlinkDirectoryIfItExist", async function (): Promise<void> {
  const testDir = path.join(testdataDir, "link_file_origin_3");
  const linkDir = path.join(testdataDir, "link_file_link_3");
  const testFile = path.join(testDir, "test.txt");

  await Deno.mkdir(testDir, { recursive: true });
  await Deno.writeFile(testFile, new Uint8Array());

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

Deno.test("ensureSymlinkSyncDirectoryIfItExist", function (): void {
  const testDir = path.join(testdataDir, "link_file_origin_3");
  const linkDir = path.join(testdataDir, "link_file_link_3");
  const testFile = path.join(testDir, "test.txt");

  Deno.mkdirSync(testDir, { recursive: true });
  Deno.writeFileSync(testFile, new Uint8Array());

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
