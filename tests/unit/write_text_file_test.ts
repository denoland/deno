// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
} from "./test_util.ts";

Deno.test(
  { permissions: { read: true, write: true } },
  function writeTextFileSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeTextFileSync(filename, "Hello");
    const dataRead = Deno.readTextFileSync(filename);
    assertEquals(dataRead, "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function writeTextFileSyncByUrl() {
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
    );
    Deno.writeTextFileSync(fileUrl, "Hello");
    const dataRead = Deno.readTextFileSync(fileUrl);
    assertEquals(dataRead, "Hello");

    Deno.removeSync(fileUrl, { recursive: true });
  },
);

Deno.test({ permissions: { write: true } }, function writeTextFileSyncFail() {
  const filename = "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist (hopefully).
  assertThrows(() => {
    Deno.writeTextFileSync(filename, "hello");
  }, Deno.errors.NotFound);
});

Deno.test({ permissions: { write: false } }, function writeTextFileSyncPerm() {
  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  assertThrows(() => {
    Deno.writeTextFileSync(filename, "Hello");
  }, Deno.errors.NotCapable);
});

Deno.test(
  { permissions: { read: true, write: true } },
  function writeTextFileSyncUpdateMode() {
    if (Deno.build.os !== "windows") {
      const data = "Hello";
      const filename = Deno.makeTempDirSync() + "/test.txt";
      Deno.writeTextFileSync(filename, data, { mode: 0o755 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
      Deno.writeTextFileSync(filename, data, { mode: 0o666 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
    }
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function writeTextFileSyncCreate() {
    const data = "Hello";
    const filename = Deno.makeTempDirSync() + "/test.txt";
    let caughtError = false;
    // if create turned off, the file won't be created
    try {
      Deno.writeTextFileSync(filename, data, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // Turn on create, should have no error
    Deno.writeTextFileSync(filename, data, { create: true });
    Deno.writeTextFileSync(filename, data, { create: false });
    assertEquals(Deno.readTextFileSync(filename), "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function writeTextFileSyncAppend() {
    const data = "Hello";
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeTextFileSync(filename, data);
    Deno.writeTextFileSync(filename, data, { append: true });
    assertEquals(Deno.readTextFileSync(filename), "HelloHello");
    // Now attempt overwrite
    Deno.writeTextFileSync(filename, data, { append: false });
    assertEquals(Deno.readTextFileSync(filename), "Hello");
    // append not set should also overwrite
    Deno.writeTextFileSync(filename, data);
    assertEquals(Deno.readTextFileSync(filename), "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeTextFileSuccess() {
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeTextFile(filename, "Hello");
    const dataRead = Deno.readTextFileSync(filename);
    assertEquals(dataRead, "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeTextFileByUrl() {
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
    );
    await Deno.writeTextFile(fileUrl, "Hello");
    const dataRead = Deno.readTextFileSync(fileUrl);
    assertEquals(dataRead, "Hello");

    Deno.removeSync(fileUrl, { recursive: true });
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeTextFileNotFound() {
    const filename = "/baddir/test.txt";
    // The following should fail because /baddir doesn't exist (hopefully).
    await assertRejects(async () => {
      await Deno.writeTextFile(filename, "Hello");
    }, Deno.errors.NotFound);
  },
);

Deno.test(
  { permissions: { write: false } },
  async function writeTextFilePerm() {
    const filename = "/baddir/test.txt";
    // The following should fail due to no write permission
    await assertRejects(async () => {
      await Deno.writeTextFile(filename, "Hello");
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeTextFileUpdateMode() {
    if (Deno.build.os !== "windows") {
      const data = "Hello";
      const filename = Deno.makeTempDirSync() + "/test.txt";
      await Deno.writeTextFile(filename, data, { mode: 0o755 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
      await Deno.writeTextFile(filename, data, { mode: 0o666 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
    }
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeTextFileCreate() {
    const data = "Hello";
    const filename = Deno.makeTempDirSync() + "/test.txt";
    let caughtError = false;
    // if create turned off, the file won't be created
    try {
      await Deno.writeTextFile(filename, data, { create: false });
    } catch (e) {
      caughtError = true;
      assert(e instanceof Deno.errors.NotFound);
    }
    assert(caughtError);

    // Turn on create, should have no error
    await Deno.writeTextFile(filename, data, { create: true });
    await Deno.writeTextFile(filename, data, { create: false });
    assertEquals(Deno.readTextFileSync(filename), "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeTextFileAppend() {
    const data = "Hello";
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeTextFile(filename, data);
    await Deno.writeTextFile(filename, data, { append: true });
    assertEquals(Deno.readTextFileSync(filename), "HelloHello");
    // Now attempt overwrite
    await Deno.writeTextFile(filename, data, { append: false });
    assertEquals(Deno.readTextFileSync(filename), "Hello");
    // append not set should also overwrite
    await Deno.writeTextFile(filename, data);
    assertEquals(Deno.readTextFileSync(filename), "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeTextFileStream() {
    const stream = new ReadableStream({
      pull(controller) {
        controller.enqueue("Hello");
        controller.enqueue("World");
        controller.close();
      },
    });
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeTextFile(filename, stream);
    assertEquals(Deno.readTextFileSync(filename), "HelloWorld");
  },
);
