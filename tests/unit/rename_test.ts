// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  AssertionError,
  assertIsError,
  assertThrows,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

function assertMissing(path: string) {
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

function assertFile(path: string) {
  const info = Deno.lstatSync(path);
  assert(info.isFile);
}

function assertDirectory(path: string, mode?: number) {
  const info = Deno.lstatSync(path);
  assert(info.isDirectory);
  if (Deno.build.os !== "windows" && mode !== undefined) {
    assertEquals(info.mode! & 0o777, mode & ~Deno.umask());
  }
}

Deno.test(
  { permissions: { read: true, write: true } },
  function renameSyncSuccess() {
    const testDir = Deno.makeTempDirSync();
    const oldpath = testDir + "/oldpath";
    const newpath = testDir + "/newpath";
    Deno.mkdirSync(oldpath);
    Deno.renameSync(oldpath, newpath);
    assertDirectory(newpath);
    assertMissing(oldpath);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function renameSyncWithURL() {
    const testDir = Deno.makeTempDirSync();
    const oldpath = testDir + "/oldpath";
    const newpath = testDir + "/newpath";
    Deno.mkdirSync(oldpath);
    Deno.renameSync(
      pathToAbsoluteFileUrl(oldpath),
      pathToAbsoluteFileUrl(newpath),
    );
    assertDirectory(newpath);
    assertMissing(oldpath);
  },
);

Deno.test(
  { permissions: { read: false, write: true } },
  function renameSyncReadPerm() {
    assertThrows(() => {
      const oldpath = "/oldbaddir";
      const newpath = "/newbaddir";
      Deno.renameSync(oldpath, newpath);
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: false } },
  function renameSyncWritePerm() {
    assertThrows(() => {
      const oldpath = "/oldbaddir";
      const newpath = "/newbaddir";
      Deno.renameSync(oldpath, newpath);
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function renameSuccess() {
    const testDir = Deno.makeTempDirSync();
    const oldpath = testDir + "/oldpath";
    const newpath = testDir + "/newpath";
    Deno.mkdirSync(oldpath);
    await Deno.rename(oldpath, newpath);
    assertDirectory(newpath);
    assertMissing(oldpath);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function renameWithURL() {
    const testDir = Deno.makeTempDirSync();
    const oldpath = testDir + "/oldpath";
    const newpath = testDir + "/newpath";
    Deno.mkdirSync(oldpath);
    await Deno.rename(
      pathToAbsoluteFileUrl(oldpath),
      pathToAbsoluteFileUrl(newpath),
    );
    assertDirectory(newpath);
    assertMissing(oldpath);
  },
);

function readFileString(filename: string): string {
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  return dec.decode(dataRead);
}

function writeFileString(filename: string, s: string) {
  const enc = new TextEncoder();
  const data = enc.encode(s);
  Deno.writeFileSync(filename, data, { mode: 0o666 });
}

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { read: true, write: true },
  },
  function renameSyncErrorsUnix() {
    const testDir = Deno.makeTempDirSync();
    const oldfile = testDir + "/oldfile";
    const olddir = testDir + "/olddir";
    const emptydir = testDir + "/empty";
    const fulldir = testDir + "/dir";
    const file = fulldir + "/file";
    writeFileString(oldfile, "Hello");
    Deno.mkdirSync(olddir);
    Deno.mkdirSync(emptydir);
    Deno.mkdirSync(fulldir);
    writeFileString(file, "world");

    assertThrows(
      () => {
        Deno.renameSync(oldfile, emptydir);
      },
      Error,
      "Is a directory",
    );
    try {
      assertThrows(
        () => {
          Deno.renameSync(olddir, fulldir);
        },
        Error,
        "Directory not empty",
      );
    } catch (e) {
      // rename syscall may also return EEXIST, e.g. with XFS
      assertIsError(
        e,
        AssertionError,
        `Expected error message to include "Directory not empty", but got "File exists`,
      );
    }
    assertThrows(
      () => {
        Deno.renameSync(olddir, file);
      },
      Error,
      "Not a directory",
    );
    assertThrows(
      () => {
        Deno.renameSync(olddir, file);
      },
      Error,
      `rename '${olddir}' -> '${file}'`,
    );

    const fileLink = testDir + "/fileLink";
    const dirLink = testDir + "/dirLink";
    const danglingLink = testDir + "/danglingLink";
    Deno.symlinkSync(file, fileLink);
    Deno.symlinkSync(emptydir, dirLink);
    Deno.symlinkSync(testDir + "/nonexistent", danglingLink);

    assertThrows(
      () => {
        Deno.renameSync(olddir, fileLink);
      },
      Error,
      "Not a directory",
    );
    assertThrows(
      () => {
        Deno.renameSync(olddir, dirLink);
      },
      Error,
      "Not a directory",
    );
    assertThrows(
      () => {
        Deno.renameSync(olddir, danglingLink);
      },
      Error,
      "Not a directory",
    );

    // should succeed on Unix
    Deno.renameSync(olddir, emptydir);
    Deno.renameSync(oldfile, dirLink);
    Deno.renameSync(dirLink, danglingLink);
    assertFile(danglingLink);
    assertEquals("Hello", readFileString(danglingLink));
  },
);

Deno.test(
  {
    ignore: Deno.build.os !== "windows",
    permissions: { read: true, write: true },
  },
  function renameSyncErrorsWin() {
    const testDir = Deno.makeTempDirSync();
    const oldfile = testDir + "/oldfile";
    const olddir = testDir + "/olddir";
    const emptydir = testDir + "/empty";
    const fulldir = testDir + "/dir";
    const file = fulldir + "/file";
    writeFileString(oldfile, "Hello");
    Deno.mkdirSync(olddir);
    Deno.mkdirSync(emptydir);
    Deno.mkdirSync(fulldir);
    writeFileString(file, "world");

    assertThrows(
      () => {
        Deno.renameSync(oldfile, emptydir);
      },
      Deno.errors.PermissionDenied,
      "Access is denied",
    );
    assertThrows(
      () => {
        Deno.renameSync(olddir, fulldir);
      },
      Deno.errors.PermissionDenied,
      "Access is denied",
    );
    assertThrows(
      () => {
        Deno.renameSync(olddir, emptydir);
      },
      Deno.errors.PermissionDenied,
      "Access is denied",
    );
    assertThrows(
      () => {
        Deno.renameSync(olddir, emptydir);
      },
      Error,
      `rename '${olddir}' -> '${emptydir}'`,
    );

    // should succeed on Windows
    Deno.renameSync(olddir, file);
    assertDirectory(file);
  },
);
