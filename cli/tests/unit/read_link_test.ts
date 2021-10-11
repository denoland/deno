// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

unitTest(
  { permissions: { write: true, read: true } },
  function readLinkSyncSuccess() {
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
  { permissions: { write: true, read: true } },
  function readLinkSyncUrlSuccess() {
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

unitTest({ permissions: { read: false } }, function readLinkSyncPerm() {
  assertThrows(() => {
    Deno.readLinkSync("/symlink");
  }, Deno.errors.PermissionDenied);
});

unitTest({ permissions: { read: true } }, function readLinkSyncNotFound() {
  assertThrows(
    () => {
      Deno.readLinkSync("bad_filename");
    },
    Deno.errors.NotFound,
    `readlink 'bad_filename'`,
  );
});

unitTest(
  { permissions: { write: true, read: true } },
  async function readLinkSuccess() {
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
  { permissions: { write: true, read: true } },
  async function readLinkUrlSuccess() {
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

unitTest({ permissions: { read: false } }, async function readLinkPerm() {
  await assertRejects(async () => {
    await Deno.readLink("/symlink");
  }, Deno.errors.PermissionDenied);
});

unitTest({ permissions: { read: true } }, async function readLinkNotFound() {
  await assertRejects(
    async () => {
      await Deno.readLink("bad_filename");
    },
    Deno.errors.NotFound,
    `readlink 'bad_filename'`,
  );
});
