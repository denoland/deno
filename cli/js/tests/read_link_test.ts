// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

unitTest(
  { perms: { write: true, read: true } },
  function readlinkSyncSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    // TODO Add test for Windows once symlink is implemented for Windows.
    // See https://github.com/denoland/deno/issues/815.
    if (Deno.build.os !== "win") {
      Deno.symlinkSync(target, symlink);
      const targetPath = Deno.readlinkSync(symlink);
      assertEquals(targetPath, target);
    }
  }
);

unitTest({ perms: { read: false } }, function readlinkSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.readlinkSync("/symlink");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function readlinkSyncNotFound(): void {
  let caughtError = false;
  let data;
  try {
    data = Deno.readlinkSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
  assertEquals(data, undefined);
});

unitTest(
  { perms: { write: true, read: true } },
  async function readlinkSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    // TODO Add test for Windows once symlink is implemented for Windows.
    // See https://github.com/denoland/deno/issues/815.
    if (Deno.build.os !== "win") {
      Deno.symlinkSync(target, symlink);
      const targetPath = await Deno.readlink(symlink);
      assertEquals(targetPath, target);
    }
  }
);

unitTest({ perms: { read: false } }, async function readlinkPerm(): Promise<
  void
> {
  let caughtError = false;
  try {
    await Deno.readlink("/symlink");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});
