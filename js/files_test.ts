// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as deno from "deno";
import { test, assert, assertEqual } from "./test_util.ts";

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
