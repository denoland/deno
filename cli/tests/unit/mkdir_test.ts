// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

function assertDirectory(path: string, mode?: number): void {
  const info = Deno.lstatSync(path);
  assert(info.isDirectory);
  if (Deno.build.os !== "windows" && mode !== undefined) {
    assertEquals(info.mode! & 0o777, mode & ~Deno.umask());
  }
}

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncSuccess(): void {
    const path = Deno.makeTempDirSync() + "/dir";
    Deno.mkdirSync(path);
    assertDirectory(path);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncMode(): void {
    const path = Deno.makeTempDirSync() + "/dir";
    Deno.mkdirSync(path, { mode: 0o737 });
    assertDirectory(path, 0o737);
  },
);

unitTest({ perms: { write: false } }, function mkdirSyncPerm(): void {
  assertThrows(() => {
    Deno.mkdirSync("/baddir");
  }, Deno.errors.PermissionDenied);
});

unitTest(
  { perms: { read: true, write: true } },
  async function mkdirSuccess(): Promise<void> {
    const path = Deno.makeTempDirSync() + "/dir";
    await Deno.mkdir(path);
    assertDirectory(path);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function mkdirMode(): Promise<void> {
    const path = Deno.makeTempDirSync() + "/dir";
    await Deno.mkdir(path, { mode: 0o737 });
    assertDirectory(path, 0o737);
  },
);

unitTest({ perms: { write: true } }, function mkdirErrSyncIfExists(): void {
  assertThrows(() => {
    Deno.mkdirSync(".");
  }, Deno.errors.AlreadyExists);
});

unitTest({ perms: { write: true } }, async function mkdirErrIfExists(): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    await Deno.mkdir(".");
  }, Deno.errors.AlreadyExists);
});

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncRecursive(): void {
    const path = Deno.makeTempDirSync() + "/nested/directory";
    Deno.mkdirSync(path, { recursive: true });
    assertDirectory(path);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function mkdirRecursive(): Promise<void> {
    const path = Deno.makeTempDirSync() + "/nested/directory";
    await Deno.mkdir(path, { recursive: true });
    assertDirectory(path);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncRecursiveMode(): void {
    const nested = Deno.makeTempDirSync() + "/nested";
    const path = nested + "/dir";
    Deno.mkdirSync(path, { mode: 0o737, recursive: true });
    assertDirectory(path, 0o737);
    assertDirectory(nested, 0o737);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function mkdirRecursiveMode(): Promise<void> {
    const nested = Deno.makeTempDirSync() + "/nested";
    const path = nested + "/dir";
    await Deno.mkdir(path, { mode: 0o737, recursive: true });
    assertDirectory(path, 0o737);
    assertDirectory(nested, 0o737);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncRecursiveIfExists(): void {
    const path = Deno.makeTempDirSync() + "/dir";
    Deno.mkdirSync(path, { mode: 0o737 });
    Deno.mkdirSync(path, { recursive: true });
    Deno.mkdirSync(path, { recursive: true, mode: 0o731 });
    assertDirectory(path, 0o737);
    if (Deno.build.os !== "windows") {
      const pathLink = path + "Link";
      Deno.symlinkSync(path, pathLink);
      Deno.mkdirSync(pathLink, { recursive: true });
      Deno.mkdirSync(pathLink, { recursive: true, mode: 0o731 });
      assertDirectory(path, 0o737);
    }
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function mkdirRecursiveIfExists(): Promise<void> {
    const path = Deno.makeTempDirSync() + "/dir";
    await Deno.mkdir(path, { mode: 0o737 });
    await Deno.mkdir(path, { recursive: true });
    await Deno.mkdir(path, { recursive: true, mode: 0o731 });
    assertDirectory(path, 0o737);
    if (Deno.build.os !== "windows") {
      const pathLink = path + "Link";
      Deno.symlinkSync(path, pathLink);
      await Deno.mkdir(pathLink, { recursive: true });
      await Deno.mkdir(pathLink, { recursive: true, mode: 0o731 });
      assertDirectory(path, 0o737);
    }
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncErrors(): void {
    const testDir = Deno.makeTempDirSync();
    const emptydir = testDir + "/empty";
    const fulldir = testDir + "/dir";
    const file = fulldir + "/file";
    Deno.mkdirSync(emptydir);
    Deno.mkdirSync(fulldir);
    Deno.createSync(file).close();

    assertThrows((): void => {
      Deno.mkdirSync(emptydir, { recursive: false });
    }, Deno.errors.AlreadyExists);
    assertThrows((): void => {
      Deno.mkdirSync(fulldir, { recursive: false });
    }, Deno.errors.AlreadyExists);
    assertThrows((): void => {
      Deno.mkdirSync(file, { recursive: false });
    }, Deno.errors.AlreadyExists);
    assertThrows((): void => {
      Deno.mkdirSync(file, { recursive: true });
    }, Deno.errors.AlreadyExists);

    if (Deno.build.os !== "windows") {
      const fileLink = testDir + "/fileLink";
      const dirLink = testDir + "/dirLink";
      const danglingLink = testDir + "/danglingLink";
      Deno.symlinkSync(file, fileLink);
      Deno.symlinkSync(emptydir, dirLink);
      Deno.symlinkSync(testDir + "/nonexistent", danglingLink);

      assertThrows((): void => {
        Deno.mkdirSync(dirLink, { recursive: false });
      }, Deno.errors.AlreadyExists);
      assertThrows((): void => {
        Deno.mkdirSync(fileLink, { recursive: false });
      }, Deno.errors.AlreadyExists);
      assertThrows((): void => {
        Deno.mkdirSync(fileLink, { recursive: true });
      }, Deno.errors.AlreadyExists);
      assertThrows((): void => {
        Deno.mkdirSync(danglingLink, { recursive: false });
      }, Deno.errors.AlreadyExists);
      assertThrows((): void => {
        Deno.mkdirSync(danglingLink, { recursive: true });
      }, Deno.errors.AlreadyExists);
    }
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncRelativeUrlPath(): void {
    const testDir = Deno.makeTempDirSync();
    const nestedDir = testDir + "/nested";
    // Add trailing slash so base path is treated as a directory. pathToAbsoluteFileUrl removes trailing slashes.
    const path = new URL("../dir", pathToAbsoluteFileUrl(nestedDir) + "/");

    Deno.mkdirSync(nestedDir);
    Deno.mkdirSync(path);

    assertDirectory(testDir + "/dir");
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function mkdirRelativeUrlPath(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const nestedDir = testDir + "/nested";
    // Add trailing slash so base path is treated as a directory. pathToAbsoluteFileUrl removes trailing slashes.
    const path = new URL("../dir", pathToAbsoluteFileUrl(nestedDir) + "/");

    await Deno.mkdir(nestedDir);
    await Deno.mkdir(path);

    assertDirectory(testDir + "/dir");
  },
);
