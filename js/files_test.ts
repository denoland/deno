// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";

test(function filesStdioFileDescriptors() {
  assertEqual(Deno.stdin.rid, 0);
  assertEqual(Deno.stdout.rid, 1);
  assertEqual(Deno.stderr.rid, 2);
});

testPerm({ read: true }, async function filesCopyToStdout() {
  const filename = "package.json";
  const file = await Deno.open(filename);
  assert(file.rid > 2);
  const bytesWritten = await Deno.copy(Deno.stdout, file);
  const fileSize = Deno.statSync(filename).len;
  assertEqual(bytesWritten, fileSize);
  console.log("bytes written", bytesWritten);
});

testPerm({ read: true }, async function filesToAsyncIterator() {
  const filename = "tests/hello.txt";
  const file = await Deno.open(filename);

  let totalSize = 0;
  for await (const buf of Deno.toAsyncIterator(file)) {
    totalSize += buf.byteLength;
  }

  assertEqual(totalSize, 12);
});

testPerm({ write: false }, async function writePermFailure() {
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
    assertEqual(err.kind, Deno.ErrorKind.PermissionDenied);
    assertEqual(err.name, "PermissionDenied");
  }
});

testPerm({ read: false }, async function readPermFailure() {
  let caughtError = false;
  try {
    await Deno.open("package.json", "r");
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ write: false, read: false }, async function readWritePermFailure() {
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
    assertEqual(err.kind, Deno.ErrorKind.PermissionDenied);
    assertEqual(err.name, "PermissionDenied");
  }
});

testPerm({ read: true, write: true }, async function createFile() {
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

testPerm({ read: true, write: true }, async function openModeWrite() {
  const tempDir = Deno.makeTempDirSync();
  const encoder = new TextEncoder();
  const filename = tempDir + "hello.txt";
  const data = encoder.encode("Hello world!\n");

  let file = await Deno.open(filename, "w");
  // assert file was created
  let fileInfo = Deno.statSync(filename);
  assert(fileInfo.isFile());
  assertEqual(fileInfo.len, 0);
  // write some data
  await file.write(data);
  fileInfo = Deno.statSync(filename);
  assertEqual(fileInfo.len, 13);
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
  assertEqual(fileSize, 0);
  await Deno.remove(tempDir, { recursive: true });
});

testPerm({ read: true, write: true }, async function openModeWriteRead() {
  const tempDir = Deno.makeTempDirSync();
  const encoder = new TextEncoder();
  const filename = tempDir + "hello.txt";
  const data = encoder.encode("Hello world!\n");

  const file = await Deno.open(filename, "w+");
  // assert file was created
  let fileInfo = Deno.statSync(filename);
  assert(fileInfo.isFile());
  assertEqual(fileInfo.len, 0);
  // write some data
  await file.write(data);
  fileInfo = Deno.statSync(filename);
  assertEqual(fileInfo.len, 13);

  // TODO: this test is not working, I expect because
  //  file handle points to the end of file, but ATM
  //  deno has no seek implementation on Rust side
  // assert file can be read
  // const buf = new Uint8Array(20);
  // const result = await file.read(buf);
  // console.log(result.eof, result.nread);
  // assertEqual(result.nread, 13);
  // file.close();

  await Deno.remove(tempDir, { recursive: true });
});
