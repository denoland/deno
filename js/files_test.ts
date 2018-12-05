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

testPerm({write: true}, async function createFile() {
  const tempDir = await deno.makeTempDir();
  const filename = tempDir + "/test.txt";
  // TODO: replace with OpenMode enum
  let f = await deno.open(filename, 'w');
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