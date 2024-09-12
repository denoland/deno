// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects, assertThrows } from "./test_util.ts";

function readFileString(filename: string | URL): string {
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  return dec.decode(dataRead);
}

function writeFileString(filename: string | URL, s: string) {
  const enc = new TextEncoder();
  const data = enc.encode(s);
  Deno.writeFileSync(filename, data, { mode: 0o666 });
}

function assertSameContent(
  filename1: string | URL,
  filename2: string | URL,
) {
  const data1 = Deno.readFileSync(filename1);
  const data2 = Deno.readFileSync(filename2);
  assertEquals(data1, data2);
}

Deno.test(
  { permissions: { read: true, write: true } },
  function copyFileSyncSuccess() {
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

Deno.test(
  { permissions: { read: true, write: true } },
  function copyFileSyncByUrl() {
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

Deno.test(
  { permissions: { write: true, read: true } },
  function copyFileSyncFailure() {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    // We skip initial writing here, from.txt does not exist
    assertThrows(
      () => {
        Deno.copyFileSync(fromFilename, toFilename);
      },
      Deno.errors.NotFound,
      `copy '${fromFilename}' -> '${toFilename}'`,
    );

    Deno.removeSync(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { write: true, read: false } },
  function copyFileSyncPerm1() {
    assertThrows(() => {
      Deno.copyFileSync("/from.txt", "/to.txt");
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { write: false, read: true } },
  function copyFileSyncPerm2() {
    assertThrows(() => {
      Deno.copyFileSync("/from.txt", "/to.txt");
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function copyFileSyncOverwrite() {
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

Deno.test(
  { permissions: { read: true, write: true } },
  async function copyFileSuccess() {
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

Deno.test(
  { permissions: { read: true, write: true } },
  async function copyFileByUrl() {
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

Deno.test(
  { permissions: { read: true, write: true } },
  async function copyFileFailure() {
    const tempDir = Deno.makeTempDirSync();
    const fromFilename = tempDir + "/from.txt";
    const toFilename = tempDir + "/to.txt";
    // We skip initial writing here, from.txt does not exist
    await assertRejects(
      async () => {
        await Deno.copyFile(fromFilename, toFilename);
      },
      Deno.errors.NotFound,
      `copy '${fromFilename}' -> '${toFilename}'`,
    );

    Deno.removeSync(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function copyFileOverwrite() {
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

Deno.test(
  { permissions: { read: false, write: true } },
  async function copyFilePerm1() {
    await assertRejects(async () => {
      await Deno.copyFile("/from.txt", "/to.txt");
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: false } },
  async function copyFilePerm2() {
    await assertRejects(async () => {
      await Deno.copyFile("/from.txt", "/to.txt");
    }, Deno.errors.NotCapable);
  },
);

function copyFileSyncMode(content: string): void {
  const tempDir = Deno.makeTempDirSync();
  const fromFilename = tempDir + "/from.txt";
  const toFilename = tempDir + "/to.txt";
  Deno.writeTextFileSync(fromFilename, content);
  Deno.chmodSync(fromFilename, 0o100755);

  Deno.copyFileSync(fromFilename, toFilename);
  const toStat = Deno.statSync(toFilename);
  assertEquals(toStat.mode!, 0o100755);
}

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { read: true, write: true },
  },
  function copyFileSyncChmod() {
    // this Tests different optimization paths on MacOS:
    //
    // < 128 KB clonefile() w/ fallback to copyfile()
    // > 128 KB
    copyFileSyncMode("Hello world!");
    copyFileSyncMode("Hello world!".repeat(128 * 1024));
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function copyFileNulPath() {
    const fromFilename = "from.txt\0";
    const toFilename = "to.txt\0";
    await assertRejects(async () => {
      await Deno.copyFile(fromFilename, toFilename);
    }, TypeError);
  },
);
