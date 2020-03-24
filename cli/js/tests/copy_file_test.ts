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
