// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEquals } from "./test_util.ts";

test(function filesStdioFileDescriptors(): void {
  assertEquals(Deno.stdin.rid, 0);
  assertEquals(Deno.stdout.rid, 1);
  assertEquals(Deno.stderr.rid, 2);
});

testPerm({ read: true }, async function filesCopyToStdout(): Promise<void> {
  const filename = "package.json";
  const file = await Deno.open(filename);
  assert(file.rid > 2);
  const bytesWritten = await Deno.copy(Deno.stdout, file);
  const fileSize = Deno.statSync(filename).len;
  assertEquals(bytesWritten, fileSize);
  console.log("bytes written", bytesWritten);
});

testPerm({ read: true }, async function filesToAsyncIterator(): Promise<void> {
  const filename = "tests/hello.txt";
  const file = await Deno.open(filename);

  let totalSize = 0;
  for await (const buf of Deno.toAsyncIterator(file)) {
    totalSize += buf.byteLength;
  }

  assertEquals(totalSize, 12);
});

testPerm({ write: false }, async function writePermFailure(): Promise<void> {
  const filename = "tests/hello.txt";
  const writeModes: Deno.OpenMode[] = ["w", "a", "x"];
  for (const mode of writeModes) {
    let err;
    try {
      await Deno.open(filename, mode);
    } catch (e) {
      err = e;
    }
    assert(!!err);
    assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(err.name, "PermissionDenied");
  }
});

testPerm({ read: false }, async function readPermFailure(): Promise<void> {
  let caughtError = false;
  try {
    await Deno.open("package.json", "r");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm(
  { write: false, read: false },
  async function readWritePermFailure(): Promise<void> {
    const filename = "tests/hello.txt";
    const writeModes: Deno.OpenMode[] = ["r+", "w+", "a+", "x+"];
    for (const mode of writeModes) {
      let err;
      try {
        await Deno.open(filename, mode);
      } catch (e) {
        err = e;
      }
      assert(!!err);
      assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
      assertEquals(err.name, "PermissionDenied");
    }
  }
);

testPerm({ read: true, write: true }, async function createFile(): Promise<
  void
> {
  const tempDir = await Deno.makeTempDir();
  const filename = tempDir + "/test.txt";
  const f = await Deno.open(filename, "w");
  let fileInfo = Deno.statSync(filename);
  assert(fileInfo.isFile());
  assert(fileInfo.len === 0);
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  await f.write(data);
  fileInfo = Deno.statSync(filename);
  assert(fileInfo.len === 5);
  f.close();

  // TODO: test different modes
  await Deno.remove(tempDir, { recursive: true });
});

testPerm({ read: true, write: true }, async function openModeWrite(): Promise<
  void
> {
  const tempDir = Deno.makeTempDirSync();
  const encoder = new TextEncoder();
  const filename = tempDir + "hello.txt";
  const data = encoder.encode("Hello world!\n");

  let file = await Deno.open(filename, "w");
  // assert file was created
  let fileInfo = Deno.statSync(filename);
  assert(fileInfo.isFile());
  assertEquals(fileInfo.len, 0);
  // write some data
  await file.write(data);
  fileInfo = Deno.statSync(filename);
  assertEquals(fileInfo.len, 13);
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
  file = await Deno.open(filename, "w");
  file.close();
  const fileSize = Deno.statSync(filename).len;
  assertEquals(fileSize, 0);
  await Deno.remove(tempDir, { recursive: true });
});

testPerm(
  { read: true, write: true },
  async function openModeWriteRead(): Promise<void> {
    const tempDir = Deno.makeTempDirSync();
    const encoder = new TextEncoder();
    const filename = tempDir + "hello.txt";
    const data = encoder.encode("Hello world!\n");

    const file = await Deno.open(filename, "w+");
    // assert file was created
    let fileInfo = Deno.statSync(filename);
    assert(fileInfo.isFile());
    assertEquals(fileInfo.len, 0);
    // write some data
    await file.write(data);
    fileInfo = Deno.statSync(filename);
    assertEquals(fileInfo.len, 13);

    const buf = new Uint8Array(20);
    await file.seek(0, Deno.SeekMode.SEEK_START);
    const result = await file.read(buf);
    assertEquals(result.nread, 13);
    file.close();

    await Deno.remove(tempDir, { recursive: true });
  }
);

testPerm({ read: true }, async function seekStart(): Promise<void> {
  const filename = "tests/hello.txt";
  const file = await Deno.open(filename);
  // Deliberately move 1 step forward
  await file.read(new Uint8Array(1)); // "H"
  // Skipping "Hello "
  await file.seek(6, Deno.SeekMode.SEEK_START);
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

testPerm({ read: true }, function seekSyncStart(): void {
  const filename = "tests/hello.txt";
  const file = Deno.openSync(filename);
  // Deliberately move 1 step forward
  file.readSync(new Uint8Array(1)); // "H"
  // Skipping "Hello "
  file.seekSync(6, Deno.SeekMode.SEEK_START);
  const buf = new Uint8Array(6);
  file.readSync(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

testPerm({ read: true }, async function seekCurrent(): Promise<void> {
  const filename = "tests/hello.txt";
  const file = await Deno.open(filename);
  // Deliberately move 1 step forward
  await file.read(new Uint8Array(1)); // "H"
  // Skipping "ello "
  await file.seek(5, Deno.SeekMode.SEEK_CURRENT);
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

testPerm({ read: true }, function seekSyncCurrent(): void {
  const filename = "tests/hello.txt";
  const file = Deno.openSync(filename);
  // Deliberately move 1 step forward
  file.readSync(new Uint8Array(1)); // "H"
  // Skipping "ello "
  file.seekSync(5, Deno.SeekMode.SEEK_CURRENT);
  const buf = new Uint8Array(6);
  file.readSync(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

testPerm({ read: true }, async function seekEnd(): Promise<void> {
  const filename = "tests/hello.txt";
  const file = await Deno.open(filename);
  await file.seek(-6, Deno.SeekMode.SEEK_END);
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

testPerm({ read: true }, function seekSyncEnd(): void {
  const filename = "tests/hello.txt";
  const file = Deno.openSync(filename);
  file.seekSync(-6, Deno.SeekMode.SEEK_END);
  const buf = new Uint8Array(6);
  file.readSync(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

testPerm({ read: true }, async function seekMode(): Promise<void> {
  const filename = "tests/hello.txt";
  const file = await Deno.open(filename);
  let err;
  try {
    await file.seek(1, -1);
  } catch (e) {
    err = e;
  }
  assert(!!err);
  assertEquals(err.kind, Deno.ErrorKind.InvalidSeekMode);
  assertEquals(err.name, "InvalidSeekMode");

  // We should still be able to read the file
  // since it is still open.
  let buf = new Uint8Array(1);
  await file.read(buf); // "H"
  assertEquals(new TextDecoder().decode(buf), "H");
});
