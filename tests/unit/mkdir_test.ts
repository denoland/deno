// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

function assertDirectory(path: string, mode?: number) {
  const info = Deno.lstatSync(path);
  assert(info.isDirectory);
  if (Deno.build.os !== "windows" && mode !== undefined) {
    assertEquals(info.mode! & 0o777, mode & ~Deno.umask());
  }
}

Deno.test(
  { permissions: { read: true, write: true } },
  function mkdirSyncSuccess() {
    const path = Deno.makeTempDirSync() + "/dir";
    Deno.mkdirSync(path);
    assertDirectory(path);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function mkdirSyncMode() {
    const path = Deno.makeTempDirSync() + "/dir";
    Deno.mkdirSync(path, { mode: 0o737 });
    assertDirectory(path, 0o737);
  },
);

Deno.test({ permissions: { write: false } }, function mkdirSyncPerm() {
  assertThrows(() => {
    Deno.mkdirSync("/baddir");
  }, Deno.errors.NotCapable);
});

Deno.test(
  { permissions: { read: true, write: true } },
  async function mkdirSuccess() {
    const path = Deno.makeTempDirSync() + "/dir";
    await Deno.mkdir(path);
    assertDirectory(path);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function mkdirMode() {
    const path = Deno.makeTempDirSync() + "/dir";
    await Deno.mkdir(path, { mode: 0o737 });
    assertDirectory(path, 0o737);
  },
);

Deno.test({ permissions: { write: true } }, function mkdirErrSyncIfExists() {
  assertThrows(
    () => {
      Deno.mkdirSync(".");
    },
    Deno.errors.AlreadyExists,
    `mkdir '.'`,
  );
});

Deno.test({ permissions: { write: true } }, async function mkdirErrIfExists() {
  await assertRejects(
    async () => {
      await Deno.mkdir(".");
    },
    Deno.errors.AlreadyExists,
    `mkdir '.'`,
  );
});

Deno.test(
  { permissions: { read: true, write: true } },
  function mkdirSyncRecursive() {
    const path = Deno.makeTempDirSync() + "/nested/directory";
    Deno.mkdirSync(path, { recursive: true });
    assertDirectory(path);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function mkdirRecursive() {
    const path = Deno.makeTempDirSync() + "/nested/directory";
    await Deno.mkdir(path, { recursive: true });
    assertDirectory(path);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function mkdirSyncRecursiveMode() {
    const nested = Deno.makeTempDirSync() + "/nested";
    const path = nested + "/dir";
    Deno.mkdirSync(path, { mode: 0o737, recursive: true });
    assertDirectory(path, 0o737);
    assertDirectory(nested, 0o737);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function mkdirRecursiveMode() {
    const nested = Deno.makeTempDirSync() + "/nested";
    const path = nested + "/dir";
    await Deno.mkdir(path, { mode: 0o737, recursive: true });
    assertDirectory(path, 0o737);
    assertDirectory(nested, 0o737);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function mkdirSyncRecursiveIfExists() {
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

Deno.test(
  { permissions: { read: true, write: true } },
  async function mkdirRecursiveIfExists() {
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

Deno.test(
  { permissions: { read: true, write: true } },
  function mkdirSyncErrors() {
    const testDir = Deno.makeTempDirSync();
    const emptydir = testDir + "/empty";
    const fulldir = testDir + "/dir";
    const file = fulldir + "/file";
    Deno.mkdirSync(emptydir);
    Deno.mkdirSync(fulldir);
    Deno.createSync(file).close();

    assertThrows(() => {
      Deno.mkdirSync(emptydir, { recursive: false });
    }, Deno.errors.AlreadyExists);
    assertThrows(() => {
      Deno.mkdirSync(fulldir, { recursive: false });
    }, Deno.errors.AlreadyExists);
    assertThrows(() => {
      Deno.mkdirSync(file, { recursive: false });
    }, Deno.errors.AlreadyExists);
    assertThrows(() => {
      Deno.mkdirSync(file, { recursive: true });
    }, Deno.errors.AlreadyExists);

    if (Deno.build.os !== "windows") {
      const fileLink = testDir + "/fileLink";
      const dirLink = testDir + "/dirLink";
      const danglingLink = testDir + "/danglingLink";
      Deno.symlinkSync(file, fileLink);
      Deno.symlinkSync(emptydir, dirLink);
      Deno.symlinkSync(testDir + "/nonexistent", danglingLink);

      assertThrows(() => {
        Deno.mkdirSync(dirLink, { recursive: false });
      }, Deno.errors.AlreadyExists);
      assertThrows(() => {
        Deno.mkdirSync(fileLink, { recursive: false });
      }, Deno.errors.AlreadyExists);
      assertThrows(() => {
        Deno.mkdirSync(fileLink, { recursive: true });
      }, Deno.errors.AlreadyExists);
      assertThrows(() => {
        Deno.mkdirSync(danglingLink, { recursive: false });
      }, Deno.errors.AlreadyExists);
      assertThrows(() => {
        Deno.mkdirSync(danglingLink, { recursive: true });
      }, Deno.errors.AlreadyExists);
    }
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function mkdirSyncRelativeUrlPath() {
    const testDir = Deno.makeTempDirSync();
    const nestedDir = testDir + "/nested";
    // Add trailing slash so base path is treated as a directory. pathToAbsoluteFileUrl removes trailing slashes.
    const path = new URL("../dir", pathToAbsoluteFileUrl(nestedDir) + "/");

    Deno.mkdirSync(nestedDir);
    Deno.mkdirSync(path);

    assertDirectory(testDir + "/dir");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function mkdirRelativeUrlPath() {
    const testDir = Deno.makeTempDirSync();
    const nestedDir = testDir + "/nested";
    // Add trailing slash so base path is treated as a directory. pathToAbsoluteFileUrl removes trailing slashes.
    const path = new URL("../dir", pathToAbsoluteFileUrl(nestedDir) + "/");

    await Deno.mkdir(nestedDir);
    await Deno.mkdir(path);

    assertDirectory(testDir + "/dir");
  },
);
