// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// TODO(axetroy): Add test for Windows once symlink is implemented for Windows.
import { test } from "../testing/mod.ts";
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync
} from "../testing/asserts.ts";
import { ensureSymlink, ensureSymlinkSync } from "./ensure_symlink.ts";
import * as path from "./path/mod.ts";

const testdataDir = path.resolve("fs", "testdata");
const isWindows = Deno.build.os === "win";

test(async function ensureSymlinkIfItNotExist(): Promise<void> {
  const testDir = path.join(testdataDir, "link_file_1");
  const testFile = path.join(testDir, "test.txt");

  assertThrowsAsync(
    async (): Promise<void> => {
      await ensureSymlink(testFile, path.join(testDir, "test1.txt"));
    }
  );

  assertThrowsAsync(
    async (): Promise<void> => {
      await Deno.stat(testFile).then((): void => {
        throw new Error("test file should exists.");
      });
    }
  );
});

test(function ensureSymlinkSyncIfItNotExist(): void {
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

test(async function ensureSymlinkIfItExist(): Promise<void> {
  const testDir = path.join(testdataDir, "link_file_3");
  const testFile = path.join(testDir, "test.txt");
  const linkFile = path.join(testDir, "link.txt");

  await Deno.mkdir(testDir, true);
  await Deno.writeFile(testFile, new Uint8Array());

  if (isWindows) {
    await assertThrowsAsync(
      (): Promise<void> => ensureSymlink(testFile, linkFile),
      Error,
      "Not implemented"
    );
    await Deno.remove(testDir, { recursive: true });
    return;
  } else {
    await ensureSymlink(testFile, linkFile);
  }

  const srcStat = await Deno.lstat(testFile);
  const linkStat = await Deno.lstat(linkFile);

  assertEquals(srcStat.isFile(), true);
  assertEquals(linkStat.isSymlink(), true);

  await Deno.remove(testDir, { recursive: true });
});

test(function ensureSymlinkSyncIfItExist(): void {
  const testDir = path.join(testdataDir, "link_file_4");
  const testFile = path.join(testDir, "test.txt");
  const linkFile = path.join(testDir, "link.txt");

  Deno.mkdirSync(testDir, true);
  Deno.writeFileSync(testFile, new Uint8Array());

  if (isWindows) {
    assertThrows(
      (): void => ensureSymlinkSync(testFile, linkFile),
      Error,
      "Not implemented"
    );
    Deno.removeSync(testDir, { recursive: true });
    return;
  } else {
    ensureSymlinkSync(testFile, linkFile);
  }

  const srcStat = Deno.lstatSync(testFile);

  const linkStat = Deno.lstatSync(linkFile);

  assertEquals(srcStat.isFile(), true);
  assertEquals(linkStat.isSymlink(), true);

  Deno.removeSync(testDir, { recursive: true });
});

test(async function ensureSymlinkDirectoryIfItExist(): Promise<void> {
  const testDir = path.join(testdataDir, "link_file_origin_3");
  const linkDir = path.join(testdataDir, "link_file_link_3");
  const testFile = path.join(testDir, "test.txt");

  await Deno.mkdir(testDir, true);
  await Deno.writeFile(testFile, new Uint8Array());

  if (isWindows) {
    await assertThrowsAsync(
      (): Promise<void> => ensureSymlink(testDir, linkDir),
      Error,
      "Not implemented"
    );
    await Deno.remove(testDir, { recursive: true });
    return;
  } else {
    await ensureSymlink(testDir, linkDir);
  }

  const testDirStat = await Deno.lstat(testDir);
  const linkDirStat = await Deno.lstat(linkDir);
  const testFileStat = await Deno.lstat(testFile);

  assertEquals(testFileStat.isFile(), true);
  assertEquals(testDirStat.isDirectory(), true);
  assertEquals(linkDirStat.isSymlink(), true);

  await Deno.remove(linkDir, { recursive: true });
  await Deno.remove(testDir, { recursive: true });
});

test(function ensureSymlinkSyncDirectoryIfItExist(): void {
  const testDir = path.join(testdataDir, "link_file_origin_3");
  const linkDir = path.join(testdataDir, "link_file_link_3");
  const testFile = path.join(testDir, "test.txt");

  Deno.mkdirSync(testDir, true);
  Deno.writeFileSync(testFile, new Uint8Array());

  if (isWindows) {
    assertThrows(
      (): void => ensureSymlinkSync(testDir, linkDir),
      Error,
      "Not implemented"
    );
    Deno.removeSync(testDir, { recursive: true });
    return;
  } else {
    ensureSymlinkSync(testDir, linkDir);
  }

  const testDirStat = Deno.lstatSync(testDir);
  const linkDirStat = Deno.lstatSync(linkDir);
  const testFileStat = Deno.lstatSync(testFile);

  assertEquals(testFileStat.isFile(), true);
  assertEquals(testDirStat.isDirectory(), true);
  assertEquals(linkDirStat.isSymlink(), true);

  Deno.removeSync(linkDir, { recursive: true });
  Deno.removeSync(testDir, { recursive: true });
});
