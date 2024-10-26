// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  unreachable,
} from "./test_util.ts";

Deno.test(
  { permissions: { read: true, write: true } },
  function writeFileSyncSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data);
    const dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function writeFileSyncUrl() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
    );
    Deno.writeFileSync(fileUrl, data);
    const dataRead = Deno.readFileSync(fileUrl);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");

    Deno.removeSync(tempDir, { recursive: true });
  },
);

Deno.test({ permissions: { write: true } }, function writeFileSyncFail() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist (hopefully).
  assertThrows(() => {
    Deno.writeFileSync(filename, data);
  }, Deno.errors.NotFound);
});

Deno.test({ permissions: { write: false } }, function writeFileSyncPerm() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  assertThrows(() => {
    Deno.writeFileSync(filename, data);
  }, Deno.errors.NotCapable);
});

Deno.test(
  { permissions: { read: true, write: true } },
  function writeFileSyncUpdateMode() {
    if (Deno.build.os !== "windows") {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const filename = Deno.makeTempDirSync() + "/test.txt";
      Deno.writeFileSync(filename, data, { mode: 0o755 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
      Deno.writeFileSync(filename, data, { mode: 0o666 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
    }
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function writeFileSyncCreate() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    // if create turned off, the file won't be created
    assertThrows(() => {
      Deno.writeFileSync(filename, data, { create: false });
    }, Deno.errors.NotFound);

    // Turn on create, should have no error
    Deno.writeFileSync(filename, data, { create: true });
    Deno.writeFileSync(filename, data, { create: false });
    const dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function writeFileSyncCreateNew() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data, { createNew: true });

    assertThrows(() => {
      Deno.writeFileSync(filename, data, { createNew: true });
    }, Deno.errors.AlreadyExists);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function writeFileSyncAppend() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data);
    Deno.writeFileSync(filename, data, { append: true });
    let dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    let actual = dec.decode(dataRead);
    assertEquals(actual, "HelloHello");
    // Now attempt overwrite
    Deno.writeFileSync(filename, data, { append: false });
    dataRead = Deno.readFileSync(filename);
    actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");
    // append not set should also overwrite
    Deno.writeFileSync(filename, data);
    dataRead = Deno.readFileSync(filename);
    actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeFile(filename, data);
    const dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileUrl() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = await Deno.makeTempDir();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
    );
    await Deno.writeFile(fileUrl, data);
    const dataRead = Deno.readFileSync(fileUrl);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");

    Deno.removeSync(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileNotFound() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = "/baddir/test.txt";
    // The following should fail because /baddir doesn't exist (hopefully).
    await assertRejects(async () => {
      await Deno.writeFile(filename, data);
    }, Deno.errors.NotFound);
  },
);

Deno.test(
  { permissions: { read: true, write: false } },
  async function writeFilePerm() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = "/baddir/test.txt";
    // The following should fail due to no write permission
    await assertRejects(async () => {
      await Deno.writeFile(filename, data);
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileUpdateMode() {
    if (Deno.build.os !== "windows") {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const filename = Deno.makeTempDirSync() + "/test.txt";
      await Deno.writeFile(filename, data, { mode: 0o755 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
      await Deno.writeFile(filename, data, { mode: 0o666 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
    }
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileCreate() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    // if create turned off, the file won't be created
    await assertRejects(async () => {
      await Deno.writeFile(filename, data, { create: false });
    }, Deno.errors.NotFound);

    // Turn on create, should have no error
    await Deno.writeFile(filename, data, { create: true });
    await Deno.writeFile(filename, data, { create: false });
    const dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileCreateNew() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeFile(filename, data, { createNew: true });
    await assertRejects(async () => {
      await Deno.writeFile(filename, data, { createNew: true });
    }, Deno.errors.AlreadyExists);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileAppend() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeFile(filename, data);
    await Deno.writeFile(filename, data, { append: true });
    let dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    let actual = dec.decode(dataRead);
    assertEquals(actual, "HelloHello");
    // Now attempt overwrite
    await Deno.writeFile(filename, data, { append: false });
    dataRead = Deno.readFileSync(filename);
    actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");
    // append not set should also overwrite
    await Deno.writeFile(filename, data);
    dataRead = Deno.readFileSync(filename);
    actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileAbortSignal(): Promise<void> {
    const ac = new AbortController();
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    queueMicrotask(() => ac.abort());
    try {
      await Deno.writeFile(filename, data, { signal: ac.signal });
      unreachable();
    } catch (e) {
      assert(e instanceof Error);
      assertEquals(e.name, "AbortError");
    }
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileAbortSignalReason(): Promise<void> {
    const ac = new AbortController();
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    const abortReason = new Error();
    queueMicrotask(() => ac.abort(abortReason));
    try {
      await Deno.writeFile(filename, data, { signal: ac.signal });
      unreachable();
    } catch (e) {
      assertEquals(e, abortReason);
    }
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileAbortSignalPrimitiveReason(): Promise<void> {
    const ac = new AbortController();
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    queueMicrotask(() => ac.abort("Some string"));
    try {
      await Deno.writeFile(filename, data, { signal: ac.signal });
      unreachable();
    } catch (e) {
      assertEquals(e, "Some string");
    }
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileAbortSignalPreAborted(): Promise<void> {
    const ac = new AbortController();
    ac.abort();
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    try {
      await Deno.writeFile(filename, data, { signal: ac.signal });
      unreachable();
    } catch (e) {
      assert(e instanceof Error);
      assertEquals(e.name, "AbortError");
    }
    assertNotExists(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileAbortSignalReasonPreAborted(): Promise<void> {
    const ac = new AbortController();
    const abortReason = new Error();
    ac.abort(abortReason);
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    try {
      await Deno.writeFile(filename, data, { signal: ac.signal });
      unreachable();
    } catch (e) {
      assertEquals(e, abortReason);
    }
    assertNotExists(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileAbortSignalPrimitiveReasonPreAborted(): Promise<
    void
  > {
    const ac = new AbortController();
    ac.abort("Some string");
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    try {
      await Deno.writeFile(filename, data, { signal: ac.signal });
      unreachable();
    } catch (e) {
      assertEquals(e, "Some string");
    }
    assertNotExists(filename);
  },
);

// Test that AbortController's cancel handle is cleaned-up correctly, and do not leak resources.
Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileWithAbortSignalNotCalled() {
    const ac = new AbortController();
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeFile(filename, data, { signal: ac.signal });
    const dataRead = Deno.readFileSync(filename);
    const dec = new TextDecoder("utf-8");
    const actual = dec.decode(dataRead);
    assertEquals(actual, "Hello");
  },
);

function assertNotExists(filename: string | URL) {
  if (pathExists(filename)) {
    throw new Error(`The file ${filename} exists.`);
  }
}

function pathExists(path: string | URL) {
  try {
    Deno.statSync(path);
    return true;
  } catch {
    return false;
  }
}

Deno.test(
  { permissions: { read: true, write: true } },
  async function writeFileStream() {
    const stream = new ReadableStream({
      pull(controller) {
        controller.enqueue(new Uint8Array([1]));
        controller.enqueue(new Uint8Array([2]));
        controller.close();
      },
    });
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeFile(filename, stream);
    assertEquals(Deno.readFileSync(filename), new Uint8Array([1, 2]));
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function overwriteFileWithStream() {
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeFile(filename, new Uint8Array([1, 2, 3, 4]));

    const stream = new ReadableStream({
      pull(controller) {
        controller.enqueue(new Uint8Array([1]));
        controller.enqueue(new Uint8Array([2]));
        controller.close();
      },
    });
    await Deno.writeFile(filename, stream);
    assertEquals(Deno.readFileSync(filename), new Uint8Array([1, 2]));
  },
);
