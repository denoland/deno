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

function assertFile(path: string): void {
  const info = Deno.lstatSync(path);
  assert(info.isFile());
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

function readFileString(filename: string): string {
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  return dec.decode(dataRead);
}

function writeFileString(filename: string, s: string): void {
  const enc = new TextEncoder();
  const data = enc.encode(s);
  Deno.writeFileSync(filename, data, { mode: 0o666 });
}

unitTest(
  { perms: { read: true, write: true } },
  function renameSyncCreateNewFile(): void {
    const testDir = Deno.makeTempDirSync();
    const oldfile = testDir + "/oldfile";
    const goodpath = testDir + "/goodpath";
    const badpath = testDir + "/badpath";
    writeFileString(oldfile, "Hello");
    writeFileString(badpath, "");
    Deno.renameSync(oldfile, goodpath, { createNew: true });
    assertMissing(oldfile);
    assertFile(goodpath);
    assertEquals("Hello", readFileString(goodpath));

    let caughtErr = false;
    try {
      Deno.renameSync(goodpath, badpath, { createNew: true });
    } catch (e) {
      caughtErr = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtErr);

    Deno.renameSync(goodpath, badpath, { createNew: false });
    assertMissing(goodpath);
    assertFile(badpath);
    assertEquals("Hello", readFileString(badpath));
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function renameCreateNewFile(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const oldfile = testDir + "/oldfile";
    const goodpath = testDir + "/goodpath";
    const badpath = testDir + "/badpath";
    writeFileString(oldfile, "Hello");
    writeFileString(badpath, "");
    await Deno.rename(oldfile, goodpath, { createNew: true });
    assertMissing(oldfile);
    assertFile(goodpath);
    assertEquals("Hello", readFileString(goodpath));

    let caughtErr = false;
    try {
      await Deno.rename(goodpath, badpath, { createNew: true });
    } catch (e) {
      caughtErr = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtErr);

    await Deno.rename(goodpath, badpath, { createNew: false });
    assertMissing(goodpath);
    assertFile(badpath);
    assertEquals("Hello", readFileString(badpath));
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function renameSyncCreateNewDir(): void {
    const testDir = Deno.makeTempDirSync();
    const olddir = testDir + "/olddir";
    const goodpath = testDir + "/goodpath";
    const badpath = testDir + "/badpath";
    Deno.mkdirSync(olddir, { mode: 0o701 });
    Deno.mkdirSync(badpath);

    Deno.renameSync(olddir, goodpath, { createNew: true });
    assertMissing(olddir);
    assertDirectory(goodpath, 0o701);

    let caughtErr = false;
    try {
      Deno.renameSync(goodpath, badpath, { createNew: true });
    } catch (e) {
      caughtErr = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtErr);

    if (Deno.build.os === "win") {
      // Windows won't overwrite a directory
      caughtErr = false;
      try {
        Deno.renameSync(goodpath, badpath, { createNew: false });
      } catch (e) {
        caughtErr = true;
        assert(e instanceof Deno.errors.PermissionDenied);
      }
      assert(caughtErr);
      Deno.removeSync(badpath);
    }

    Deno.renameSync(goodpath, badpath, { createNew: false });
    assertMissing(goodpath);
    assertDirectory(badpath, 0o701);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function renameCreateNewDir(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const olddir = testDir + "/olddir";
    const goodpath = testDir + "/goodpath";
    const badpath = testDir + "/badpath";
    Deno.mkdirSync(olddir, { mode: 0o701 });
    Deno.mkdirSync(badpath);

    await Deno.rename(olddir, goodpath, { createNew: true });
    assertMissing(olddir);
    assertDirectory(goodpath, 0o701);

    let caughtErr = false;
    try {
      await Deno.rename(goodpath, badpath, { createNew: true });
    } catch (e) {
      caughtErr = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtErr);

    if (Deno.build.os === "win") {
      // Windows won't overwrite a directory
      caughtErr = false;
      try {
        await Deno.rename(goodpath, badpath, { createNew: false });
      } catch (e) {
        caughtErr = true;
        assert(e instanceof Deno.errors.PermissionDenied);
      }
      assert(caughtErr);
      Deno.removeSync(badpath);
    }

    await Deno.rename(goodpath, badpath, { createNew: false });
    assertMissing(goodpath);
    assertDirectory(badpath, 0o701);
  }
);
