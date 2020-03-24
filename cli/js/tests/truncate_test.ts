// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals, assert } from "./test_util.ts";

function readDataSync(name: string): string {
  const data = Deno.readFileSync(name);
  const decoder = new TextDecoder("utf-8");
  const text = decoder.decode(data);
  return text;
}

async function readData(name: string): Promise<string> {
  const data = await Deno.readFile(name);
  const decoder = new TextDecoder("utf-8");
  const text = decoder.decode(data);
  return text;
}

function assertFile(path: string, size?: number, mode?: number): void {
  const info = Deno.lstatSync(path);
  assert(info.isFile());
  if (size !== undefined) {
    assertEquals(info.size, size);
  }
  if (Deno.build.os !== "win" && mode !== undefined) {
    assertEquals(info.mode! & 0o777, mode & ~Deno.umask());
  }
}

unitTest(
  { perms: { read: true, write: true } },
  function truncateSyncSuccess(): void {
    const enc = new TextEncoder();
    const d = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test_truncateSync.txt";
    Deno.writeFileSync(filename, d);
    Deno.truncateSync(filename, 20);
    let data = readDataSync(filename);
    assertEquals(data.length, 20);
    Deno.truncateSync(filename, 5);
    data = readDataSync(filename);
    assertEquals(data.length, 5);
    Deno.truncateSync(filename, -5);
    data = readDataSync(filename);
    assertEquals(data.length, 0);
    Deno.removeSync(filename);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function truncateSuccess(): Promise<void> {
    const enc = new TextEncoder();
    const d = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test_truncate.txt";
    await Deno.writeFile(filename, d);
    await Deno.truncate(filename, 20);
    let data = await readData(filename);
    assertEquals(data.length, 20);
    await Deno.truncate(filename, 5);
    data = await readData(filename);
    assertEquals(data.length, 5);
    await Deno.truncate(filename, -5);
    data = await readData(filename);
    assertEquals(data.length, 0);
    await Deno.remove(filename);
  }
);

unitTest(
  {
    perms: { read: true, write: true },
  },
  function truncateSyncMode(): void {
    const path = Deno.makeTempDirSync() + "/test_truncateSync.txt";
    Deno.truncateSync(path, 20, { mode: 0o626 });
    assertFile(path, 20, 0o626);
  }
);

unitTest(
  {
    perms: { read: true, write: true },
  },
  async function truncateMode(): Promise<void> {
    const path = (await Deno.makeTempDir()) + "/test_truncate.txt";
    await Deno.truncate(path, 20, { mode: 0o626 });
    assertFile(path, 20, 0o626);
  }
);

unitTest({ perms: { write: false } }, function truncateSyncPerm(): void {
  let err;
  try {
    Deno.mkdirSync("/test_truncateSyncPermission.txt");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

unitTest({ perms: { write: false } }, async function truncatePerm(): Promise<
  void
> {
  let err;
  try {
    await Deno.mkdir("/test_truncatePermission.txt");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

unitTest(
  { perms: { read: true, write: true } },
  function truncateSyncCreate(): void {
    const filename = Deno.makeTempDirSync() + "/test.txt";
    let caughtError = false;
    // if create turned off, the file won't be created
    try {
      Deno.truncateSync(filename, 0, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // Turn on create, should have no error
    Deno.truncateSync(filename, 10, { create: true });
    assertFile(filename, 10);
    Deno.truncateSync(filename, 0, { create: false });
    assertFile(filename, 0);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function truncateCreate(): Promise<void> {
    const filename = Deno.makeTempDirSync() + "/test.txt";
    let caughtError = false;
    // if create turned off, the file won't be created
    try {
      await Deno.truncate(filename, 0, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // Turn on create, should have no error
    await Deno.truncate(filename, 10, { create: true });
    assertFile(filename, 10);
    await Deno.truncate(filename, 0, { create: false });
    assertFile(filename, 0);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function truncateSyncCreateNew(): void {
    const filename = Deno.makeTempDirSync() + "/test.txt";
    // file newly created
    Deno.truncateSync(filename, 0, { createNew: true });
    // createNew: true but file exists
    let caughtError = false;
    try {
      Deno.truncateSync(filename, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    // createNew: false and file exists
    Deno.truncateSync(filename, 10, { createNew: false });
    assertFile(filename, 10);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function truncateCreateNew(): Promise<void> {
    const filename = Deno.makeTempDirSync() + "/test.txt";
    // file newly created
    await Deno.truncate(filename, 0, { createNew: true });
    // createNew: true but file exists
    let caughtError = false;
    try {
      await Deno.truncate(filename, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    // createNew: false and file exists
    await Deno.truncate(filename, 10, { createNew: false });
    assertFile(filename, 10);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function truncateSyncDir(): void {
    const testDir = Deno.makeTempDirSync();
    const dir = testDir + "/dir";
    Deno.mkdirSync(dir);
    let caughtError = false;
    try {
      Deno.truncateSync(dir, 0);
    } catch (e) {
      caughtError = true;
      if (Deno.build.os == "win") {
        assert(e instanceof Deno.errors.PermissionDenied);
      } else {
        assert(e.message.includes("Is a directory"));
      }
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.truncateSync(dir, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function truncateDir(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const dir = testDir + "/dir";
    Deno.mkdirSync(dir);
    let caughtError = false;
    try {
      await Deno.truncate(dir, 0);
    } catch (e) {
      caughtError = true;
      if (Deno.build.os == "win") {
        assert(e instanceof Deno.errors.PermissionDenied);
      } else {
        assert(e.message.includes("Is a directory"));
      }
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.truncate(dir, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
  }
);

function assertLink(path: string, valid: boolean): void {
  let info = Deno.lstatSync(path);
  assert(info.isSymlink());
  let caughtErr = false;
  try {
    info = Deno.statSync(path);
  } catch (e) {
    caughtErr = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  if (valid) {
    assert(!caughtErr);
  } else {
    assert(caughtErr);
    assertEquals(info, undefined);
  }
}

unitTest(
  { ignore: Deno.build.os === "win", perms: { read: true, write: true } },
  function truncateSyncLinks(): void {
    const testDir = Deno.makeTempDirSync();
    const dir = testDir + "/dir";
    const file = testDir + "/file";
    Deno.mkdirSync(dir);
    Deno.createSync(file).close();
    const fileLink = testDir + "/fileLink";
    const dirLink = testDir + "/dirLink";
    const danglingLink = testDir + "/danglingLink";
    const danglingTarget = testDir + "/nonexistent";
    Deno.symlinkSync(file, fileLink);
    Deno.symlinkSync(dir, dirLink);
    Deno.symlinkSync(danglingTarget, danglingLink);
    let caughtError = false;
    try {
      Deno.truncateSync(fileLink, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.truncateSync(dirLink, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.truncateSync(danglingLink, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.truncateSync(dirLink, 0);
    } catch (e) {
      caughtError = true;
      assert(e.message.includes("Is a directory"));
    }
    assert(caughtError);
    // should succeed
    Deno.truncateSync(fileLink, 10);
    assertLink(fileLink, true);
    assertFile(file, 10);
    Deno.truncateSync(danglingLink, 20);
    assertLink(danglingLink, true);
    assertFile(danglingTarget, 20);
  }
);

unitTest(
  { ignore: Deno.build.os === "win", perms: { read: true, write: true } },
  async function truncateLinks(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const dir = testDir + "/dir";
    const file = testDir + "/file";
    Deno.mkdirSync(dir);
    Deno.createSync(file).close();
    const fileLink = testDir + "/fileLink";
    const dirLink = testDir + "/dirLink";
    const danglingLink = testDir + "/danglingLink";
    const danglingTarget = testDir + "/nonexistent";
    Deno.symlinkSync(file, fileLink);
    Deno.symlinkSync(dir, dirLink);
    Deno.symlinkSync(danglingTarget, danglingLink);
    let caughtError = false;
    try {
      await Deno.truncate(fileLink, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.truncate(dirLink, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.truncate(danglingLink, 0, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.truncate(dirLink, 0);
    } catch (e) {
      caughtError = true;
      assert(e.message.includes("Is a directory"));
    }
    assert(caughtError);
    // should succeed
    await Deno.truncate(fileLink, 10);
    assertLink(fileLink, true);
    assertFile(file, 10);
    await Deno.truncate(danglingLink, 20);
    assertLink(danglingLink, true);
    assertFile(danglingTarget, 20);
  }
);
