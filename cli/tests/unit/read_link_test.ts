// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

unitTest(
  { perms: { write: true, read: true } },
  function readLinkSyncSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const target = testDir +
      (Deno.build.os == "windows" ? "\\target" : "/target");
    const symlink = testDir +
      (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = Deno.readLinkSync(symlink);
    assertEquals(targetPath, target);
  },
);

unitTest(
  { perms: { write: true, read: true } },
  function readLinkSyncUrlSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const target = testDir +
      (Deno.build.os == "windows" ? "\\target" : "/target");
    const symlink = testDir +
      (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = Deno.readLinkSync(pathToAbsoluteFileUrl(symlink));
    assertEquals(targetPath, target);
  },
);

unitTest({ perms: { read: false } }, function readLinkSyncPerm(): void {
  assertThrows(() => {
    Deno.readLinkSync("/symlink");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function readLinkSyncNotFound(): void {
  assertThrows(() => {
    Deno.readLinkSync("bad_filename");
  }, Deno.errors.NotFound);
});

unitTest(
  { perms: { write: true, read: true } },
  async function readLinkSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const target = testDir +
      (Deno.build.os == "windows" ? "\\target" : "/target");
    const symlink = testDir +
      (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = await Deno.readLink(symlink);
    assertEquals(targetPath, target);
  },
);

unitTest(
  { perms: { write: true, read: true } },
  async function readLinkUrlSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const target = testDir +
      (Deno.build.os == "windows" ? "\\target" : "/target");
    const symlink = testDir +
      (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
    Deno.mkdirSync(target);
    Deno.symlinkSync(target, symlink);
    const targetPath = await Deno.readLink(pathToAbsoluteFileUrl(symlink));
    assertEquals(targetPath, target);
  },
);

unitTest({ perms: { read: false } }, async function readLinkPerm(): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    await Deno.readLink("/symlink");
  }, Deno.errors.PermissionDenied);
});
