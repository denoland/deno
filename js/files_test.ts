// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as deno from "deno";
import { test, testPerm, assert, assertEqual } from "./test_util.ts";

test(function filesStdioFileDescriptors() {
  assertEqual(deno.stdin.rid, 0);
  assertEqual(deno.stdout.rid, 1);
  assertEqual(deno.stderr.rid, 2);
});

test(async function filesCopyToStdout() {
  const filename = "package.json";
  const file = await deno.open(filename);
  assert(file.rid > 2);
  const bytesWritten = await deno.copy(deno.stdout, file);
  const fileSize = deno.statSync(filename).len;
  assertEqual(bytesWritten, fileSize);
  console.log("bytes written", bytesWritten);
});

test(async function filesToAsyncIterator() {
  const filename = "tests/hello.txt";
  const file = await deno.open(filename);

  let totalSize = 0;
  for await (const buf of deno.toAsyncIterator(file)) {
    totalSize += buf.byteLength;
  }

  assertEqual(totalSize, 12);
});

testPerm({ write: false }, async function writePermFailure() {
  const filename = "tests/hello.txt";
  const writeModes: deno.OpenMode[] = ["r+", "w", "w+", "a", "a+", "x", "x+"];
  for (const mode of writeModes) {
    let err;
    try {
      await deno.open(filename, mode);
    } catch (e) {
      err = e;
    }
    assert(!!err);
    assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
    assertEqual(err.name, "PermissionDenied");
  }
});

testPerm({ write: true }, async function createFile() {
  const tempDir = await deno.makeTempDir();
  const filename = tempDir + "/test.txt";
  const f = await deno.open(filename, "w");
  let fileInfo = deno.statSync(filename);
  assert(fileInfo.isFile());
  assert(fileInfo.len === 0);
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  await f.write(data);
  fileInfo = deno.statSync(filename);
  assert(fileInfo.len === 5);
  f.close();

  // TODO: test different modes
  await deno.removeAll(tempDir);
});

testPerm({ write: true }, async function openModeWrite() {
  const tempDir = deno.makeTempDirSync();
  const encoder = new TextEncoder();
  const filename = tempDir + "hello.txt";
  const data = encoder.encode("Hello world!\n");

  let file = await deno.open(filename, "w");
  // assert file was created
  let fileInfo = deno.statSync(filename);
  assert(fileInfo.isFile());
  assertEqual(fileInfo.len, 0);
  // write some data
  await file.write(data);
  fileInfo = deno.statSync(filename);
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
  file = await deno.open(filename, "w");
  file.close();
  const fileSize = deno.statSync(filename).len;
  assertEqual(fileSize, 0);
  await deno.removeAll(tempDir);
});

testPerm({ write: true }, async function openModeWriteRead() {
  const tempDir = deno.makeTempDirSync();
  const encoder = new TextEncoder();
  const filename = tempDir + "hello.txt";
  const data = encoder.encode("Hello world!\n");

  const file = await deno.open(filename, "w+");
  // assert file was created
  let fileInfo = deno.statSync(filename);
  assert(fileInfo.isFile());
  assertEqual(fileInfo.len, 0);
  // write some data
  await file.write(data);
  fileInfo = deno.statSync(filename);
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

  await deno.removeAll(tempDir);
});
