// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function writeFileSyncSuccess(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data);
    const dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function writeFileSyncUrl(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`
    );
    Deno.writeFileSync(fileUrl, data);
    const dataRead = Deno.readFileSync(fileUrl);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals("Hello", actual);

    Deno.removeSync(tempDir, { recursive: true });
  }
);

unitTest({ perms: { write: true } }, function writeFileSyncFail(): void {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist (hopefully).
  let caughtError = false;
  try {
    Deno.writeFileSync(filename, data);
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
});

unitTest({ perms: { write: false } }, function writeFileSyncPerm(): void {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  let caughtError = false;
  try {
    Deno.writeFileSync(filename, data);
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest(
  { perms: { read: true, write: true } },
  function writeFileSyncUpdateMode(): void {
    if (Deno.build.os !== "windows") {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const filename = Deno.makeTempDirSync() + "/test.txt";
      Deno.writeFileSync(filename, data, { mode: 0o755 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
      Deno.writeFileSync(filename, data, { mode: 0o666 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
    }
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function writeFileSyncCreate(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    let caughtError = false;
    // if create turned off, the file won't be created
    try {
      Deno.writeFileSync(filename, data, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // Turn on create, should have no error
    Deno.writeFileSync(filename, data, { create: true });
    Deno.writeFileSync(filename, data, { create: false });
    const dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function writeFileSyncAppend(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data);
    Deno.writeFileSync(filename, data, { append: true });
    let dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    let actual = dec.decode(dataRead);
    assertEquals("HelloHello", actual);
    // Now attempt overwrite
    Deno.writeFileSync(filename, data, { append: false });
    dataRead = Deno.readFileSync(filename);
    actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
    // append not set should also overwrite
    Deno.writeFileSync(filename, data);
    dataRead = Deno.readFileSync(filename);
    actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeFileSuccess(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeFile(filename, data);
    const dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeFileUrl(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = await Deno.makeTempDir();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`
    );
    await Deno.writeFile(fileUrl, data);
    const dataRead = Deno.readFileSync(fileUrl);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals("Hello", actual);

    Deno.removeSync(tempDir, { recursive: true });
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeFileNotFound(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = "/baddir/test.txt";
    // The following should fail because /baddir doesn't exist (hopefully).
    let caughtError = false;
    try {
      await Deno.writeFile(filename, data);
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);
  }
);

unitTest(
  { perms: { read: true, write: false } },
  async function writeFilePerm(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = "/baddir/test.txt";
    // The following should fail due to no write permission
    let caughtError = false;
    try {
      await Deno.writeFile(filename, data);
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.PermissionDenied);
    }
    assert(caughtError);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeFileUpdateMode(): Promise<void> {
    if (Deno.build.os !== "windows") {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const filename = Deno.makeTempDirSync() + "/test.txt";
      await Deno.writeFile(filename, data, { mode: 0o755 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
      await Deno.writeFile(filename, data, { mode: 0o666 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
    }
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeFileCreate(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    let caughtError = false;
    // if create turned off, the file won't be created
    try {
      await Deno.writeFile(filename, data, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // Turn on create, should have no error
    await Deno.writeFile(filename, data, { create: true });
    await Deno.writeFile(filename, data, { create: false });
    const dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeFileAppend(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeFile(filename, data);
    await Deno.writeFile(filename, data, { append: true });
    let dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    let actual = dec.decode(dataRead);
    assertEquals("HelloHello", actual);
    // Now attempt overwrite
    await Deno.writeFile(filename, data, { append: false });
    dataRead = Deno.readFileSync(filename);
    actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
    // append not set should also overwrite
    await Deno.writeFile(filename, data);
    dataRead = Deno.readFileSync(filename);
    actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
  }
);
