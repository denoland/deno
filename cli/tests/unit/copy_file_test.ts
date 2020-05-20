// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

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

function assertSameContent(filename1: string, filename2: string): void {
  const data1 = Deno.readFileSync(filename1);
  const data2 = Deno.readFileSync(filename2);
  assertEquals(data1, data2);
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
