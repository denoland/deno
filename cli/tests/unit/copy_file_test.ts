// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, assertThrowsAsync } from "./test_util.ts";

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

Deno.test("copyFileSyncSuccess", function (): void {
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
});

Deno.test("copyFileSyncByUrl", function (): void {
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
});

Deno.test("copyFileSyncFailure", function (): void {
  const tempDir = Deno.makeTempDirSync();
  const fromFilename = tempDir + "/from.txt";
  const toFilename = tempDir + "/to.txt";
  // We skip initial writing here, from.txt does not exist
  assertThrows(() => {
    Deno.copyFileSync(fromFilename, toFilename);
  }, Deno.errors.NotFound);

  Deno.removeSync(tempDir, { recursive: true });
});

Deno.test("copyFileSyncOverwrite", function (): void {
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
});

Deno.test("copyFileSuccess", async function (): Promise<void> {
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
});

Deno.test("copyFileByUrl", async function (): Promise<void> {
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
});

Deno.test("copyFileFailure", async function (): Promise<void> {
  const tempDir = Deno.makeTempDirSync();
  const fromFilename = tempDir + "/from.txt";
  const toFilename = tempDir + "/to.txt";
  // We skip initial writing here, from.txt does not exist
  await assertThrowsAsync(async () => {
    await Deno.copyFile(fromFilename, toFilename);
  }, Deno.errors.NotFound);

  Deno.removeSync(tempDir, { recursive: true });
});

Deno.test("copyFileOverwrite", async function (): Promise<void> {
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
});

Deno.test("copyFilePerm1", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  await assertThrowsAsync(async () => {
    await Deno.copyFile("/from.txt", "/to.txt");
  }, Deno.errors.PermissionDenied);
});

Deno.test("copyFileSyncPerm1", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  assertThrows(() => {
    Deno.copyFileSync("/from.txt", "/to.txt");
  }, Deno.errors.PermissionDenied);
});

Deno.test("copyFilePerm2", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "write" });

  await assertThrowsAsync(async () => {
    await Deno.copyFile("/from.txt", "/to.txt");
  }, Deno.errors.PermissionDenied);
});

Deno.test("copyFileSyncPerm2", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "write" });

  assertThrows(() => {
    Deno.copyFileSync("/from.txt", "/to.txt");
  }, Deno.errors.PermissionDenied);
});
