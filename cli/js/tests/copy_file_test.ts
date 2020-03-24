// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

function readFileString(filename: string): string {
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  return dec.decode(dataRead);
}

function writeFileString(filename: string, s: string, mode = 0o666): void {
  const enc = new TextEncoder();
  const data = enc.encode(s);
  Deno.writeFileSync(filename, data, { mode });
}

function assertSameContent(filename1: string, filename2: string): void {
  const data1 = Deno.readFileSync(filename1);
  const data2 = Deno.readFileSync(filename2);
  assertEquals(data1, data2);
}

function assertFile(path: string, mode?: number): void {
  const info = Deno.lstatSync(path);
  assert(info.isFile());
  if (Deno.build.os !== "win" && mode !== undefined) {
    // when writeFile is reimplemented in terms of open, it will respect umask
    // assertEquals(info.mode, mode & ~Deno.umask());
    assertEquals(info.mode! & 0o777, mode);
  }
}

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
  { perms: { read: true, write: true } },
  function copyFileSyncSuccess(): void {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    writeFileString(fromFilename, "Hello world!");
    Deno.copyFileSync(fromFilename, toFilename);
    // No change to original file
    assertEquals(readFileString(fromFilename), "Hello world!");
    // Original == Dest
    assertSameContent(fromFilename, toFilename);
  }
);

unitTest(
  { perms: { write: true, read: true } },
  function copyFileSyncFailure(): void {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    // We skip initial writing here, from.txt does not exist
    let err;
    try {
      Deno.copyFileSync(fromFilename, toFilename);
    } catch (e) {
      err = e;
    }
    assert(!!err);
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { write: true, read: false } },
  function copyFileSyncPerm1(): void {
    let caughtError = false;
    try {
      Deno.copyFileSync("/from.txt", "/to.txt");
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.PermissionDenied);
    }
    assert(caughtError);
  }
);

unitTest(
  { perms: { write: false, read: true } },
  function copyFileSyncPerm2(): void {
    let caughtError = false;
    try {
      Deno.copyFileSync("/from.txt", "/to.txt");
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.PermissionDenied);
    }
    assert(caughtError);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function copyFileSyncOverwrite(): void {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    writeFileString(fromFilename, "Hello world!");
    // Make Dest exist and have different content
    writeFileString(toFilename, "Goodbye!");
    Deno.copyFileSync(fromFilename, toFilename);
    // No change to original file
    assertEquals(readFileString(fromFilename), "Hello world!");
    // Original == Dest
    assertSameContent(fromFilename, toFilename);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function copyFileSuccess(): Promise<void> {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    writeFileString(fromFilename, "Hello world!");
    await Deno.copyFile(fromFilename, toFilename);
    // No change to original file
    assertEquals(readFileString(fromFilename), "Hello world!");
    // Original == Dest
    assertSameContent(fromFilename, toFilename);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function copyFileFailure(): Promise<void> {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    // We skip initial writing here, from.txt does not exist
    let err;
    try {
      await Deno.copyFile(fromFilename, toFilename);
    } catch (e) {
      err = e;
    }
    assert(!!err);
    assert(err instanceof Deno.errors.NotFound);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function copyFileOverwrite(): Promise<void> {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    writeFileString(fromFilename, "Hello world!");
    // Make Dest exist and have different content
    writeFileString(toFilename, "Goodbye!");
    await Deno.copyFile(fromFilename, toFilename);
    // No change to original file
    assertEquals(readFileString(fromFilename), "Hello world!");
    // Original == Dest
    assertSameContent(fromFilename, toFilename);
  }
);

unitTest(
  { perms: { read: false, write: true } },
  async function copyFilePerm1(): Promise<void> {
    let caughtError = false;
    try {
      await Deno.copyFile("/from.txt", "/to.txt");
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.PermissionDenied);
    }
    assert(caughtError);
  }
);

unitTest(
  { perms: { read: true, write: false } },
  async function copyFilePerm2(): Promise<void> {
    let caughtError = false;
    try {
      await Deno.copyFile("/from.txt", "/to.txt");
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.PermissionDenied);
    }
    assert(caughtError);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function copyFileSyncCreate(): void {
    const testDir = Deno.makeTempDirSync();
    const from = testDir + "/from.txt";
    const alt = testDir + "/alt.txt";
    const to = testDir + "/to.txt";
    writeFileString(from, "Hello", 0o626);
    writeFileString(alt, "world", 0o660);

    let caughtError = false;
    // if create turned off, the file won't be copied
    try {
      Deno.copyFileSync(from, to, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // Turn on create, should have no error
    Deno.copyFileSync(from, to, { create: true });
    assertFile(to, 0o626);
    assertSameContent(from, to);
    assertEquals(readFileString(to), "Hello");
    Deno.copyFileSync(alt, to, { create: false });
    assertFile(to, 0o660);
    assertSameContent(alt, to);
    assertEquals(readFileString(to), "world");
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function copyFileCreate(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const from = testDir + "/from.txt";
    const alt = testDir + "/alt.txt";
    const to = testDir + "/to.txt";
    writeFileString(from, "Hello", 0o626);
    writeFileString(alt, "world", 0o660);

    let caughtError = false;
    // if create turned off, the file won't be copied
    try {
      await Deno.copyFile(from, to, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // Turn on create, should have no error
    await Deno.copyFile(from, to, { create: true });
    assertFile(to, 0o626);
    assertSameContent(from, to);
    assertEquals(readFileString(to), "Hello");
    await Deno.copyFile(alt, to, { create: false });
    assertFile(to, 0o660);
    assertSameContent(alt, to);
    assertEquals(readFileString(to), "world");
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function copyFileSyncCreateNew(): void {
    const testDir = Deno.makeTempDirSync();
    const from = testDir + "/from.txt";
    const alt = testDir + "/alt.txt";
    const to = testDir + "/to.txt";
    writeFileString(from, "Hello", 0o626);
    writeFileString(alt, "world", 0o660);

    Deno.copyFileSync(from, to, { createNew: true });
    assertFile(to, 0o626);
    assertSameContent(from, to);
    assertEquals(readFileString(to), "Hello");

    // createNew: true but file exists
    let caughtError = false;
    try {
      Deno.copyFileSync(alt, to, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);

    // createNew: false and file exists
    Deno.copyFileSync(alt, to, { createNew: false });
    assertFile(to, 0o660);
    assertSameContent(alt, to);
    assertEquals(readFileString(to), "world");
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function copyFileCreateNew(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const from = testDir + "/from.txt";
    const alt = testDir + "/alt.txt";
    const to = testDir + "/to.txt";
    writeFileString(from, "Hello", 0o626);
    writeFileString(alt, "world", 0o660);

    await Deno.copyFile(from, to, { createNew: true });
    assertFile(to, 0o626);
    assertSameContent(from, to);
    assertEquals(readFileString(to), "Hello");

    // createNew: true but file exists
    let caughtError = false;
    try {
      await Deno.copyFile(alt, to, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);

    // createNew: false and file exists
    await Deno.copyFile(alt, to, { createNew: false });
    assertFile(to, 0o660);
    assertSameContent(alt, to);
    assertEquals(readFileString(to), "world");
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function copyFileSyncDir(): void {
    const testDir = Deno.makeTempDirSync();
    const from = testDir + "/from.txt";
    const dir = testDir + "/dir";
    writeFileString(from, "Hello");
    Deno.mkdirSync(dir);

    let caughtError = false;
    try {
      Deno.copyFileSync(from, dir);
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
      Deno.copyFileSync(from, dir, { create: false });
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
      Deno.copyFileSync(from, dir, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function copyFileDir(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const from = testDir + "/from.txt";
    const dir = testDir + "/dir";
    writeFileString(from, "Hello");
    Deno.mkdirSync(dir);

    let caughtError = false;
    try {
      await Deno.copyFile(from, dir);
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
      await Deno.copyFile(from, dir, { create: false });
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
      await Deno.copyFile(from, dir, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
  }
);

unitTest(
  { ignore: Deno.build.os === "win", perms: { read: true, write: true } },
  function copyFileSyncLinks(): void {
    const testDir = Deno.makeTempDirSync();
    const from = testDir + "/from.txt";
    const alt = testDir + "/alt.txt";
    const dir = testDir + "/dir";
    const file = testDir + "/file";
    writeFileString(from, "Hello", 0o626);
    writeFileString(alt, "world", 0o660);
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
      Deno.copyFileSync(from, fileLink, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.copyFileSync(from, dirLink, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.copyFileSync(from, dirLink, { create: true });
    } catch (e) {
      caughtError = true;
      assert(e.message.includes("Is a directory"));
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.copyFileSync(from, dirLink, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e.message.includes("Is a directory"));
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.copyFileSync(from, danglingLink, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // should succeed
    Deno.copyFileSync(from, fileLink, { create: true });
    assertLink(fileLink, true);
    assertFile(file, 0o626);
    assertSameContent(from, file);
    assertEquals(readFileString(file), "Hello");

    Deno.copyFileSync(alt, fileLink, { create: false });
    assertLink(fileLink, true);
    assertFile(file, 0o660);
    assertSameContent(alt, file);
    assertEquals(readFileString(file), "world");

    Deno.copyFileSync(from, danglingLink, { createNew: true });
    assertLink(danglingLink, true);
    assertFile(danglingTarget, 0o626);
    assertSameContent(from, danglingTarget);
    assertEquals(readFileString(danglingTarget), "Hello");

    Deno.removeSync(danglingTarget);
    Deno.copyFileSync(alt, danglingLink, { create: true });
    assertLink(danglingLink, true);
    assertFile(danglingTarget, 0o660);
    assertSameContent(alt, danglingTarget);
    assertEquals(readFileString(danglingTarget), "world");
  }
);

unitTest(
  { ignore: Deno.build.os === "win", perms: { read: true, write: true } },
  async function copyFileLinks(): Promise<void> {
    const testDir = Deno.makeTempDirSync();
    const from = testDir + "/from.txt";
    const alt = testDir + "/alt.txt";
    const dir = testDir + "/dir";
    const file = testDir + "/file";
    writeFileString(from, "Hello", 0o626);
    writeFileString(alt, "world", 0o660);
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
      await Deno.copyFile(from, fileLink, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.copyFile(from, dirLink, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.copyFile(from, dirLink, { create: true });
    } catch (e) {
      caughtError = true;
      assert(e.message.includes("Is a directory"));
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.copyFile(from, dirLink, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e.message.includes("Is a directory"));
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.copyFile(from, danglingLink, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // should succeed
    await Deno.copyFile(from, fileLink, { create: true });
    assertLink(fileLink, true);
    assertFile(file, 0o626);
    assertSameContent(from, file);
    assertEquals(readFileString(file), "Hello");

    await Deno.copyFile(alt, fileLink, { create: false });
    assertLink(fileLink, true);
    assertFile(file, 0o660);
    assertSameContent(alt, file);
    assertEquals(readFileString(file), "world");

    await Deno.copyFile(from, danglingLink, { createNew: true });
    assertLink(danglingLink, true);
    assertFile(danglingTarget, 0o626);
    assertSameContent(from, danglingTarget);
    assertEquals(readFileString(danglingTarget), "Hello");

    Deno.removeSync(danglingTarget);
    await Deno.copyFile(alt, danglingLink, { create: true });
    assertLink(danglingLink, true);
    assertFile(danglingTarget, 0o660);
    assertSameContent(alt, danglingTarget);
    assertEquals(readFileString(danglingTarget), "world");
  }
);
