// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

testPerm({ read: true, write: true }, function writeFileSyncSuccess(): void {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test.txt";
  Deno.writeFileSync(filename, data);
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEquals("Hello", actual);
});

testPerm({ write: true }, function writeFileSyncFail(): void {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist (hopefully).
  let caughtError = false;
  try {
    Deno.writeFileSync(filename, data);
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.NotFound);
    assertEquals(e.name, "NotFound");
  }
  assert(caughtError);
});

testPerm({ write: false }, function writeFileSyncPerm(): void {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  let caughtError = false;
  try {
    Deno.writeFileSync(filename, data);
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true, write: true }, function writeFileSyncUpdatePerm(): void {
  if (Deno.build.os !== "win") {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data, { perm: 0o755 });
    assertEquals(Deno.statSync(filename).mode & 0o777, 0o755);
    Deno.writeFileSync(filename, data, { perm: 0o666 });
    assertEquals(Deno.statSync(filename).mode & 0o777, 0o666);
  }
});

testPerm({ read: true, write: true }, function writeFileSyncCreate(): void {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test.txt";
  let caughtError = false;
  // if create turned off, the file won't be created
  try {
    Deno.writeFileSync(filename, data, { create: false });
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.NotFound);
    assertEquals(e.name, "NotFound");
  }
  assert(caughtError);

  // Turn on create, should have no error
  Deno.writeFileSync(filename, data, { create: true });
  Deno.writeFileSync(filename, data, { create: false });
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEquals("Hello", actual);
});

testPerm({ read: true, write: true }, function writeFileSyncAppend(): void {
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
});

testPerm(
  { read: true, write: true },
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

testPerm(
  { read: true, write: true },
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
      assertEquals(e.kind, Deno.ErrorKind.NotFound);
      assertEquals(e.name, "NotFound");
    }
    assert(caughtError);
  }
);

testPerm({ read: true, write: false }, async function writeFilePerm(): Promise<
  void
> {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  let caughtError = false;
  try {
    await Deno.writeFile(filename, data);
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm(
  { read: true, write: true },
  async function writeFileUpdatePerm(): Promise<void> {
    if (Deno.build.os !== "win") {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const filename = Deno.makeTempDirSync() + "/test.txt";
      await Deno.writeFile(filename, data, { perm: 0o755 });
      assertEquals(Deno.statSync(filename).mode & 0o777, 0o755);
      await Deno.writeFile(filename, data, { perm: 0o666 });
      assertEquals(Deno.statSync(filename).mode & 0o777, 0o666);
    }
  }
);

testPerm({ read: true, write: true }, async function writeFileCreate(): Promise<
  void
> {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test.txt";
  let caughtError = false;
  // if create turned off, the file won't be created
  try {
    await Deno.writeFile(filename, data, { create: false });
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.NotFound);
    assertEquals(e.name, "NotFound");
  }
  assert(caughtError);

  // Turn on create, should have no error
  await Deno.writeFile(filename, data, { create: true });
  await Deno.writeFile(filename, data, { create: false });
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEquals("Hello", actual);
});

testPerm({ read: true, write: true }, async function writeFileAppend(): Promise<
  void
> {
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
});
