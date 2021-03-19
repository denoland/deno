// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrowsAsync } from "./test_util.ts";

Deno.test("filesStdioFileDescriptors", function (): void {
  assertEquals(Deno.stdin.rid, 0);
  assertEquals(Deno.stdout.rid, 1);
  assertEquals(Deno.stderr.rid, 2);
});

Deno.test("filesCopyToStdout", async function (): Promise<
  void
> {
  const filename = "cli/tests/fixture.json";
  const file = await Deno.open(filename);
  assert(file.rid > 2);
  const bytesWritten = await Deno.copy(file, Deno.stdout);
  const fileSize = Deno.statSync(filename).size;
  assertEquals(bytesWritten, fileSize);
  file.close();
});

Deno.test("filesIter", async function (): Promise<void> {
  const filename = "cli/tests/hello.txt";
  const file = await Deno.open(filename);

  let totalSize = 0;
  for await (const buf of Deno.iter(file)) {
    totalSize += buf.byteLength;
  }

  assertEquals(totalSize, 12);
  file.close();
});

Deno.test("filesIterCustomBufSize", async function (): Promise<void> {
  const filename = "cli/tests/hello.txt";
  const file = await Deno.open(filename);

  let totalSize = 0;
  let iterations = 0;
  for await (const buf of Deno.iter(file, { bufSize: 6 })) {
    totalSize += buf.byteLength;
    iterations += 1;
  }

  assertEquals(totalSize, 12);
  assertEquals(iterations, 2);
  file.close();
});

Deno.test("filesIterSync", function (): void {
  const filename = "cli/tests/hello.txt";
  const file = Deno.openSync(filename);

  let totalSize = 0;
  for (const buf of Deno.iterSync(file)) {
    totalSize += buf.byteLength;
  }

  assertEquals(totalSize, 12);
  file.close();
});

Deno.test("filesIterSyncCustomBufSize", function (): void {
  const filename = "cli/tests/hello.txt";
  const file = Deno.openSync(filename);

  let totalSize = 0;
  let iterations = 0;
  for (const buf of Deno.iterSync(file, { bufSize: 6 })) {
    totalSize += buf.byteLength;
    iterations += 1;
  }

  assertEquals(totalSize, 12);
  assertEquals(iterations, 2);
  file.close();
});

Deno.test("readerIter", async function (): Promise<void> {
  // ref: https://github.com/denoland/deno/issues/2330
  const encoder = new TextEncoder();

  class TestReader implements Deno.Reader {
    #offset = 0;
    #buf: Uint8Array;

    constructor(s: string) {
      this.#buf = new Uint8Array(encoder.encode(s));
    }

    read(p: Uint8Array): Promise<number | null> {
      const n = Math.min(p.byteLength, this.#buf.byteLength - this.#offset);
      p.set(this.#buf.slice(this.#offset, this.#offset + n));
      this.#offset += n;

      if (n === 0) {
        return Promise.resolve(null);
      }

      return Promise.resolve(n);
    }
  }

  const reader = new TestReader("hello world!");

  let totalSize = 0;
  for await (const buf of Deno.iter(reader)) {
    totalSize += buf.byteLength;
  }

  assertEquals(totalSize, 12);
});

Deno.test("readerIterSync", async function (): Promise<void> {
  // ref: https://github.com/denoland/deno/issues/2330
  const encoder = new TextEncoder();

  class TestReader implements Deno.ReaderSync {
    #offset = 0;
    #buf: Uint8Array;

    constructor(s: string) {
      this.#buf = new Uint8Array(encoder.encode(s));
    }

    readSync(p: Uint8Array): number | null {
      const n = Math.min(p.byteLength, this.#buf.byteLength - this.#offset);
      p.set(this.#buf.slice(this.#offset, this.#offset + n));
      this.#offset += n;

      if (n === 0) {
        return null;
      }

      return n;
    }
  }

  const reader = new TestReader("hello world!");

  let totalSize = 0;
  for await (const buf of Deno.iterSync(reader)) {
    totalSize += buf.byteLength;
  }

  assertEquals(totalSize, 12);
});

Deno.test("openSyncMode", function (): void {
  const path = Deno.makeTempDirSync() + "/test_openSync.txt";
  const file = Deno.openSync(path, {
    write: true,
    createNew: true,
    mode: 0o626,
  });
  file.close();
  const pathInfo = Deno.statSync(path);
  if (Deno.build.os !== "windows") {
    assertEquals(pathInfo.mode! & 0o777, 0o626 & ~Deno.umask());
  }
});

Deno.test("openMode", async function (): Promise<void> {
  const path = (await Deno.makeTempDir()) + "/test_open.txt";
  const file = await Deno.open(path, {
    write: true,
    createNew: true,
    mode: 0o626,
  });
  file.close();
  const pathInfo = Deno.statSync(path);
  if (Deno.build.os !== "windows") {
    assertEquals(pathInfo.mode! & 0o777, 0o626 & ~Deno.umask());
  }
});

Deno.test("openSyncUrl", function (): void {
  const tempDir = Deno.makeTempDirSync();
  const fileUrl = new URL(
    `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test_open.txt`,
  );
  const file = Deno.openSync(fileUrl, {
    write: true,
    createNew: true,
    mode: 0o626,
  });
  file.close();
  const pathInfo = Deno.statSync(fileUrl);
  if (Deno.build.os !== "windows") {
    assertEquals(pathInfo.mode! & 0o777, 0o626 & ~Deno.umask());
  }

  Deno.removeSync(tempDir, { recursive: true });
});

Deno.test("openUrl", async function (): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  const fileUrl = new URL(
    `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test_open.txt`,
  );
  const file = await Deno.open(fileUrl, {
    write: true,
    createNew: true,
    mode: 0o626,
  });
  file.close();
  const pathInfo = Deno.statSync(fileUrl);
  if (Deno.build.os !== "windows") {
    assertEquals(pathInfo.mode! & 0o777, 0o626 & ~Deno.umask());
  }

  Deno.removeSync(tempDir, { recursive: true });
});

Deno.test("openOptions", async function (): Promise<void> {
  const filename = "cli/tests/fixture.json";
  await assertThrowsAsync(
    async (): Promise<void> => {
      await Deno.open(filename, { write: false });
    },
    Error,
    "OpenOptions requires at least one option to be true",
  );

  await assertThrowsAsync(
    async (): Promise<void> => {
      await Deno.open(filename, { truncate: true, write: false });
    },
    Error,
    "'truncate' option requires 'write' option",
  );

  await assertThrowsAsync(
    async (): Promise<void> => {
      await Deno.open(filename, { create: true, write: false });
    },
    Error,
    "'create' or 'createNew' options require 'write' or 'append' option",
  );

  await assertThrowsAsync(
    async (): Promise<void> => {
      await Deno.open(filename, { createNew: true, append: false });
    },
    Error,
    "'create' or 'createNew' options require 'write' or 'append' option",
  );
});

Deno.test("writeNullBufferFailure", async function (): Promise<void> {
  const tempDir = Deno.makeTempDirSync();
  const filename = tempDir + "hello.txt";
  const w = {
    write: true,
    truncate: true,
    create: true,
  };
  const file = await Deno.open(filename, w);

  // writing null should throw an error
  await assertThrowsAsync(
    async (): Promise<void> => {
      // deno-lint-ignore no-explicit-any
      await file.write(null as any);
    },
  ); // TODO(bartlomieju): Check error kind when dispatch_minimal pipes errors properly
  file.close();
  await Deno.remove(tempDir, { recursive: true });
});

Deno.test("readNullBufferFailure", async function (): Promise<void> {
  const tempDir = Deno.makeTempDirSync();
  const filename = tempDir + "hello.txt";
  const file = await Deno.open(filename, {
    read: true,
    write: true,
    truncate: true,
    create: true,
  });

  // reading into an empty buffer should return 0 immediately
  const bytesRead = await file.read(new Uint8Array(0));
  assert(bytesRead === 0);

  // reading file into null buffer should throw an error
  await assertThrowsAsync(async () => {
    // deno-lint-ignore no-explicit-any
    await file.read(null as any);
  }, TypeError);
  // TODO(bartlomieju): Check error kind when dispatch_minimal pipes errors properly

  file.close();
  await Deno.remove(tempDir, { recursive: true });
});

Deno.test("createFile", async function (): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  const filename = tempDir + "/test.txt";
  const f = await Deno.create(filename);
  let fileInfo = Deno.statSync(filename);
  assert(fileInfo.isFile);
  assert(fileInfo.size === 0);
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  await f.write(data);
  fileInfo = Deno.statSync(filename);
  assert(fileInfo.size === 5);
  f.close();

  // TODO(bartlomieju): test different modes
  await Deno.remove(tempDir, { recursive: true });
});

Deno.test("createFileWithUrl", async function (): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  const fileUrl = new URL(
    `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
  );
  const f = await Deno.create(fileUrl);
  let fileInfo = Deno.statSync(fileUrl);
  assert(fileInfo.isFile);
  assert(fileInfo.size === 0);
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  await f.write(data);
  fileInfo = Deno.statSync(fileUrl);
  assert(fileInfo.size === 5);
  f.close();

  await Deno.remove(tempDir, { recursive: true });
});

Deno.test("createSyncFile", async function (): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  const filename = tempDir + "/test.txt";
  const f = Deno.createSync(filename);
  let fileInfo = Deno.statSync(filename);
  assert(fileInfo.isFile);
  assert(fileInfo.size === 0);
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  await f.write(data);
  fileInfo = Deno.statSync(filename);
  assert(fileInfo.size === 5);
  f.close();

  // TODO(bartlomieju): test different modes
  await Deno.remove(tempDir, { recursive: true });
});

Deno.test("createSyncFileWithUrl", async function (): Promise<void> {
  const tempDir = await Deno.makeTempDir();
  const fileUrl = new URL(
    `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
  );
  const f = Deno.createSync(fileUrl);
  let fileInfo = Deno.statSync(fileUrl);
  assert(fileInfo.isFile);
  assert(fileInfo.size === 0);
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  await f.write(data);
  fileInfo = Deno.statSync(fileUrl);
  assert(fileInfo.size === 5);
  f.close();

  await Deno.remove(tempDir, { recursive: true });
});

Deno.test("openModeWrite", async function (): Promise<void> {
  const tempDir = Deno.makeTempDirSync();
  const encoder = new TextEncoder();
  const filename = tempDir + "hello.txt";
  const data = encoder.encode("Hello world!\n");
  let file = await Deno.open(filename, {
    create: true,
    write: true,
    truncate: true,
  });
  // assert file was created
  let fileInfo = Deno.statSync(filename);
  assert(fileInfo.isFile);
  assertEquals(fileInfo.size, 0);
  // write some data
  await file.write(data);
  fileInfo = Deno.statSync(filename);
  assertEquals(fileInfo.size, 13);
  // assert we can't read from file
  let thrown = false;
  try {
    const buf = new Uint8Array(20);
    await file.read(buf);
  } catch (e) {
    thrown = true;
  } finally {
    assert(thrown, "'w' mode shouldn't allow to read file");
  }
  file.close();
  // assert that existing file is truncated on open
  file = await Deno.open(filename, {
    write: true,
    truncate: true,
  });
  file.close();
  const fileSize = Deno.statSync(filename).size;
  assertEquals(fileSize, 0);
  await Deno.remove(tempDir, { recursive: true });
});

Deno.test("openModeWriteRead", async function (): Promise<void> {
  const tempDir = Deno.makeTempDirSync();
  const encoder = new TextEncoder();
  const filename = tempDir + "hello.txt";
  const data = encoder.encode("Hello world!\n");

  const file = await Deno.open(filename, {
    write: true,
    truncate: true,
    create: true,
    read: true,
  });
  const seekPosition = 0;
  // assert file was created
  let fileInfo = Deno.statSync(filename);
  assert(fileInfo.isFile);
  assertEquals(fileInfo.size, 0);
  // write some data
  await file.write(data);
  fileInfo = Deno.statSync(filename);
  assertEquals(fileInfo.size, 13);

  const buf = new Uint8Array(20);
  // seeking from beginning of a file
  const cursorPosition = await file.seek(seekPosition, Deno.SeekMode.Start);
  assertEquals(seekPosition, cursorPosition);
  const result = await file.read(buf);
  assertEquals(result, 13);
  file.close();

  await Deno.remove(tempDir, { recursive: true });
});

Deno.test("seekStart", async function (): Promise<void> {
  const filename = "cli/tests/hello.txt";
  const file = await Deno.open(filename);
  const seekPosition = 6;
  // Deliberately move 1 step forward
  await file.read(new Uint8Array(1)); // "H"
  // Skipping "Hello "
  // seeking from beginning of a file plus seekPosition
  const cursorPosition = await file.seek(seekPosition, Deno.SeekMode.Start);
  assertEquals(seekPosition, cursorPosition);
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
  file.close();
});

Deno.test("seekSyncStart", function (): void {
  const filename = "cli/tests/hello.txt";
  const file = Deno.openSync(filename);
  const seekPosition = 6;
  // Deliberately move 1 step forward
  file.readSync(new Uint8Array(1)); // "H"
  // Skipping "Hello "
  // seeking from beginning of a file plus seekPosition
  const cursorPosition = file.seekSync(seekPosition, Deno.SeekMode.Start);
  assertEquals(seekPosition, cursorPosition);
  const buf = new Uint8Array(6);
  file.readSync(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
  file.close();
});

Deno.test("seekCurrent", async function (): Promise<
  void
> {
  const filename = "cli/tests/hello.txt";
  const file = await Deno.open(filename);
  // Deliberately move 1 step forward
  await file.read(new Uint8Array(1)); // "H"
  // Skipping "ello "
  const seekPosition = 5;
  // seekPosition is relative to current cursor position after read
  const cursorPosition = await file.seek(seekPosition, Deno.SeekMode.Current);
  assertEquals(seekPosition + 1, cursorPosition);
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
  file.close();
});

Deno.test("seekSyncCurrent", function (): void {
  const filename = "cli/tests/hello.txt";
  const file = Deno.openSync(filename);
  // Deliberately move 1 step forward
  file.readSync(new Uint8Array(1)); // "H"
  // Skipping "ello "
  const seekPosition = 5;
  // seekPosition is relative to current cursor position after read
  const cursorPosition = file.seekSync(seekPosition, Deno.SeekMode.Current);
  assertEquals(seekPosition + 1, cursorPosition);
  const buf = new Uint8Array(6);
  file.readSync(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
  file.close();
});

Deno.test("seekEnd", async function (): Promise<void> {
  const filename = "cli/tests/hello.txt";
  const file = await Deno.open(filename);
  const seekPosition = -6;
  // seek from end of file that has 12 chars, 12 - 6  = 6
  const cursorPosition = await file.seek(seekPosition, Deno.SeekMode.End);
  assertEquals(6, cursorPosition);
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
  file.close();
});

Deno.test("seekSyncEnd", function (): void {
  const filename = "cli/tests/hello.txt";
  const file = Deno.openSync(filename);
  const seekPosition = -6;
  // seek from end of file that has 12 chars, 12 - 6  = 6
  const cursorPosition = file.seekSync(seekPosition, Deno.SeekMode.End);
  assertEquals(6, cursorPosition);
  const buf = new Uint8Array(6);
  file.readSync(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
  file.close();
});

Deno.test("seekMode", async function (): Promise<void> {
  const filename = "cli/tests/hello.txt";
  const file = await Deno.open(filename);
  await assertThrowsAsync(
    async (): Promise<void> => {
      await file.seek(1, -1);
    },
    TypeError,
    "Invalid seek mode",
  );

  // We should still be able to read the file
  // since it is still open.
  const buf = new Uint8Array(1);
  await file.read(buf); // "H"
  assertEquals(new TextDecoder().decode(buf), "H");
  file.close();
});

Deno.test("readPermFailure", async function (): Promise<
  void
> {
  await Deno.permissions.revoke({ name: "read" });

  await assertThrowsAsync(async () => {
    await Deno.open("package.json", { read: true });
  }, Deno.errors.PermissionDenied);
});

Deno.test("writePermFailure", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "write" });

  const filename = "tests/hello.txt";
  const openOptions: Deno.OpenOptions[] = [{ write: true }, { append: true }];
  for (const options of openOptions) {
    await assertThrowsAsync(async () => {
      await Deno.open(filename, options);
    }, Deno.errors.PermissionDenied);
  }
});

Deno.test("readWritePermFailure", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });
  await Deno.permissions.revoke({ name: "write" });

  const filename = "tests/hello.txt";
  await assertThrowsAsync(async () => {
    await Deno.open(filename, { read: true });
  }, Deno.errors.PermissionDenied);
});
