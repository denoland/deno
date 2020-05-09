// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

unitTest(
  { perms: { write: true, read: true } },
  function readLinkSyncSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    // TODO Add test for Windows once symlink is implemented for Windows.
    // See https://github.com/denoland/deno/issues/815.
    if (Deno.build.os !== "windows") {
      Deno.symlinkSync(target, symlink);
      const targetPath = Deno.readLinkSync(symlink);
      assertEquals(targetPath, target);
    }
  }
);

unitTest({ perms: { read: false } }, function readLinkSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.readLinkSync("/symlink");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function readLinkSyncNotFound(): void {
  let caughtError = false;
  let data;
  try {
    data = Deno.readLinkSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
  assertEquals(data, undefined);
});

unitTest(
  { perms: { write: true, read: true } },
  async function readLinkSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const target = testDir + "/target";
    const symlink = testDir + "/symln";
    Deno.mkdirSync(target);
    // TODO Add test for Windows once symlink is implemented for Windows.
    // See https://github.com/denoland/deno/issues/815.
    if (Deno.build.os !== "windows") {
      Deno.symlinkSync(target, symlink);
      const targetPath = await Deno.readLink(symlink);
      assertEquals(targetPath, target);
    }
  }
);

unitTest({ perms: { read: false } }, async function readLinkPerm(): Promise<
  void
> {
  let caughtError = false;
  try {
    await Deno.readLink("/symlink");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});
