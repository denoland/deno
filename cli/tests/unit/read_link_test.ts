// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

Deno.test("readLinkSyncSuccess", function (): void {
  const testDir = Deno.makeTempDirSync();
  const target = testDir +
    (Deno.build.os == "windows" ? "\\target" : "/target");
  const symlink = testDir +
    (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
  Deno.mkdirSync(target);
  Deno.symlinkSync(target, symlink);
  const targetPath = Deno.readLinkSync(symlink);
  assertEquals(targetPath, target);
});

Deno.test("readLinkSyncUrlSuccess", function (): void {
  const testDir = Deno.makeTempDirSync();
  const target = testDir +
    (Deno.build.os == "windows" ? "\\target" : "/target");
  const symlink = testDir +
    (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
  Deno.mkdirSync(target);
  Deno.symlinkSync(target, symlink);
  const targetPath = Deno.readLinkSync(pathToAbsoluteFileUrl(symlink));
  assertEquals(targetPath, target);
});

Deno.test("readLinkSyncNotFound", function (): void {
  assertThrows(() => {
    Deno.readLinkSync("bad_filename");
  }, Deno.errors.NotFound);
});

Deno.test("readLinkSuccess", async function (): Promise<void> {
  const testDir = Deno.makeTempDirSync();
  const target = testDir +
    (Deno.build.os == "windows" ? "\\target" : "/target");
  const symlink = testDir +
    (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
  Deno.mkdirSync(target);
  Deno.symlinkSync(target, symlink);
  const targetPath = await Deno.readLink(symlink);
  assertEquals(targetPath, target);
});

Deno.test("readLinkUrlSuccess", async function (): Promise<void> {
  const testDir = Deno.makeTempDirSync();
  const target = testDir +
    (Deno.build.os == "windows" ? "\\target" : "/target");
  const symlink = testDir +
    (Deno.build.os == "windows" ? "\\symlink" : "/symlink");
  Deno.mkdirSync(target);
  Deno.symlinkSync(target, symlink);
  const targetPath = await Deno.readLink(pathToAbsoluteFileUrl(symlink));
  assertEquals(targetPath, target);
});

Deno.test("readLinkPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  await assertThrowsAsync(async () => {
    await Deno.readLink("/symlink");
  }, Deno.errors.PermissionDenied);
});

Deno.test("readLinkSyncPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  assertThrows(() => {
    Deno.readLinkSync("/symlink");
  }, Deno.errors.PermissionDenied);
});
