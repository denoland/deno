// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

function assertMissing(path: string): void {
  let caughtErr = false;
  let info;
  try {
    info = Deno.lstatSync(path);
  } catch (e) {
    caughtErr = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtErr);
  assertEquals(info, undefined);
}

function assertDirectory(path: string, mode?: number): void {
  const info = Deno.lstatSync(path);
  assert(info.isDirectory());
  if (Deno.build.os !== "win" && mode !== undefined) {
    assertEquals(info.mode! & 0o777, mode & ~Deno.umask());
  }
}

unitTest(
  { perms: { read: true, write: true } },
  function renameSyncSuccess(): void {
    const testDir = Deno.makeTempDirSync();
    const oldpath = testDir + "/oldpath";
    const newpath = testDir + "/newpath";
    Deno.mkdirSync(oldpath);
    Deno.renameSync(oldpath, newpath);
    assertDirectory(newpath);
    assertMissing(oldpath);
  }
);

unitTest(
  { perms: { read: false, write: true } },
  function renameSyncReadPerm(): void {
    let err;
    try {
      const oldpath = "/oldbaddir";
      const newpath = "/newbaddir";
      Deno.renameSync(oldpath, newpath);
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
);

unitTest(
  { perms: { read: true, write: false } },
  function renameSyncWritePerm(): void {
    let err;
    try {
      const oldpath = "/oldbaddir";
      const newpath = "/newbaddir";
      Deno.renameSync(oldpath, newpath);
    } catch (e) {
      err = e;
    }
    assert(err instanceof Deno.errors.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function renameSuccess(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const oldpath = testDir + "/oldpath";
    const newpath = testDir + "/newpath";
    Deno.mkdirSync(oldpath);
    await Deno.rename(oldpath, newpath);
    assertDirectory(newpath);
    assertMissing(oldpath);
  }
);
