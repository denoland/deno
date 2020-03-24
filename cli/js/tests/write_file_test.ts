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

unitTest({ perms: { write: true } }, function writeFileSyncFail(): void {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist
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
    if (Deno.build.os !== "win") {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const filename = Deno.makeTempDirSync() + "/test.txt";
      Deno.writeFileSync(filename, data, { mode: 0o626 });
      assertEquals(
        Deno.statSync(filename).mode! & 0o777,
        0o626 & ~Deno.umask()
      );
      Deno.writeFileSync(filename, data, { mode: 0o737 });
      assertEquals(
        Deno.statSync(filename).mode! & 0o777,
        0o626 & ~Deno.umask()
      );
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
  function writeFileSyncCreateNew(): void {
    const enc = new TextEncoder();
    const data1 = enc.encode("Hello");
    const data2 = enc.encode("world");
    const dec = new TextDecoder("utf-8");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    // file newly created
    Deno.writeFileSync(filename, data1, { createNew: true });
    const dataRead1 = Deno.readFileSync(filename);
    const actual1 = dec.decode(dataRead1);
    assertEquals("Hello", actual1);
    // createNew: true but file exists
    let caughtError = false;
    try {
      Deno.writeFileSync(filename, data2, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    // createNew: false and file exists
    Deno.writeFileSync(filename, data2, { createNew: false });
    const dataRead2 = Deno.readFileSync(filename);
    const actual2 = dec.decode(dataRead2);
    assertEquals("world", actual2);
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
  async function writeFileNotFound(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/baddir/test.txt";
    // The following should fail because /baddir doesn't exist
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
    if (Deno.build.os !== "win") {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const filename = Deno.makeTempDirSync() + "/test.txt";
      await Deno.writeFile(filename, data, { mode: 0o626 });
      assertEquals(
        Deno.statSync(filename).mode! & 0o777,
        0o626 & ~Deno.umask()
      );
      await Deno.writeFile(filename, data, { mode: 0o737 });
      assertEquals(
        Deno.statSync(filename).mode! & 0o777,
        0o626 & ~Deno.umask()
      );
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
  async function writeFileCreateNew(): Promise<void> {
    const enc = new TextEncoder();
    const data1 = enc.encode("Hello");
    const data2 = enc.encode("world");
    const dec = new TextDecoder("utf-8");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    // file newly created
    await Deno.writeFile(filename, data1, { createNew: true });
    const dataRead1 = Deno.readFileSync(filename);
    const actual1 = dec.decode(dataRead1);
    assertEquals("Hello", actual1);
    // createNew: true but file exists
    let caughtError = false;
    try {
      await Deno.writeFile(filename, data2, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    // createNew: false and file exists
    await Deno.writeFile(filename, data2, { createNew: false });
    const dataRead2 = Deno.readFileSync(filename);
    const actual2 = dec.decode(dataRead2);
    assertEquals("world", actual2);
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

unitTest(
  { perms: { read: true, write: true } },
  function writeFileSyncDir(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const testDir = Deno.makeTempDirSync();
    const dir = testDir + "/dir";
    Deno.mkdirSync(dir);
    let caughtError = false;
    try {
      Deno.writeFileSync(dir, data);
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
      Deno.writeFileSync(dir, data, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeFileDir(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const testDir = Deno.makeTempDirSync();
    const dir = testDir + "/dir";
    Deno.mkdirSync(dir);
    let caughtError = false;
    try {
      await Deno.writeFile(dir, data);
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
      await Deno.writeFile(dir, data, { createNew: true });
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
  function writeFileSyncLinks(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
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
      Deno.writeFileSync(fileLink, data, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.writeFileSync(dirLink, data, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.writeFileSync(danglingLink, data, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      Deno.writeFileSync(dirLink, data);
    } catch (e) {
      caughtError = true;
      assert(e.message.includes("Is a directory"));
    }
    assert(caughtError);
    // should succeed
    Deno.writeFileSync(fileLink, data);
    assertLink(fileLink, true);
    Deno.writeFileSync(danglingLink, data);
    assertLink(danglingLink, true);
    const dec = new TextDecoder("utf-8");
    const dataRead = Deno.readFileSync(danglingTarget);
    const actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
  }
);

unitTest(
  { ignore: Deno.build.os === "win", perms: { read: true, write: true } },
  async function writeFileLinks(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
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
      await Deno.writeFile(fileLink, data, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.writeFile(dirLink, data, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.writeFile(danglingLink, data, { createNew: true });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.AlreadyExists);
    }
    assert(caughtError);
    caughtError = false;
    try {
      await Deno.writeFile(dirLink, data);
    } catch (e) {
      caughtError = true;
      assert(e.message.includes("Is a directory"));
    }
    assert(caughtError);
    // should succeed
    await Deno.writeFile(fileLink, data);
    assertLink(fileLink, true);
    await Deno.writeFile(danglingLink, data);
    assertLink(danglingLink, true);
    const dec = new TextDecoder("utf-8");
    const dataRead = Deno.readFileSync(danglingTarget);
    const actual = dec.decode(dataRead);
    assertEquals("Hello", actual);
  }
);
