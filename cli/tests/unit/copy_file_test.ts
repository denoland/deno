// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  unitTest,
} from "./test_util.ts";

function readFileString(filename: string | URL): string {
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  return dec.decode(dataRead);
}

function writeFileString(filename: string | URL, s: string): void {
  const enc = new TextEncoder();
  const data = enc.encode(s);
  Deno.writeFileSync(filename, data, { mode: 0o666 });
}

function assertSameContent(
  filename1: string | URL,
  filename2: string | URL,
): void {
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

    Deno.removeSync(tempDir, { recursive: true });
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function copyFileSyncByUrl(): void {
    const tempDir = Deno.makeTempDirSync();
    const fromUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/from.txt`,
    );
    const toUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/to.txt`,
    );
    writeFileString(fromUrl, "Hello world!");
    Deno.copyFileSync(fromUrl, toUrl);
    // No change to original file
    assertEquals(readFileString(fromUrl), "Hello world!");
    // Original == Dest
    assertSameContent(fromUrl, toUrl);

    Deno.removeSync(tempDir, { recursive: true });
  },
);

unitTest(
  { perms: { write: true, read: true } },
  function copyFileSyncFailure(): void {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    // We skip initial writing here, from.txt does not exist
    assertThrows(() => {
      Deno.copyFileSync(fromFilename, toFilename);
    }, Deno.errors.NotFound);

    Deno.removeSync(tempDir, { recursive: true });
  },
);

unitTest(
  { perms: { write: true, read: false } },
  function copyFileSyncPerm1(): void {
    assertThrows(() => {
      Deno.copyFileSync("/from.txt", "/to.txt");
    }, Deno.errors.PermissionDenied);
  },
);

unitTest(
  { perms: { write: false, read: true } },
  function copyFileSyncPerm2(): void {
    assertThrows(() => {
      Deno.copyFileSync("/from.txt", "/to.txt");
    }, Deno.errors.PermissionDenied);
  },
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

    Deno.removeSync(tempDir, { recursive: true });
  },
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

    Deno.removeSync(tempDir, { recursive: true });
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function copyFileByUrl(): Promise<void> {
    const tempDir = Deno.makeTempDirSync();
    const fromUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/from.txt`,
    );
    const toUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/to.txt`,
    );
    writeFileString(fromUrl, "Hello world!");
    await Deno.copyFile(fromUrl, toUrl);
    // No change to original file
    assertEquals(readFileString(fromUrl), "Hello world!");
    // Original == Dest
    assertSameContent(fromUrl, toUrl);

    Deno.removeSync(tempDir, { recursive: true });
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function copyFileFailure(): Promise<void> {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    // We skip initial writing here, from.txt does not exist
    await assertThrowsAsync(async () => {
      await Deno.copyFile(fromFilename, toFilename);
    }, Deno.errors.NotFound);

    Deno.removeSync(tempDir, { recursive: true });
  },
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

    Deno.removeSync(tempDir, { recursive: true });
  },
);

unitTest(
  { perms: { read: false, write: true } },
  async function copyFilePerm1(): Promise<void> {
    await assertThrowsAsync(async () => {
      await Deno.copyFile("/from.txt", "/to.txt");
    }, Deno.errors.PermissionDenied);
  },
);

unitTest(
  { perms: { read: true, write: false } },
  async function copyFilePerm2(): Promise<void> {
    await assertThrowsAsync(async () => {
      await Deno.copyFile("/from.txt", "/to.txt");
    }, Deno.errors.PermissionDenied);
  },
);
