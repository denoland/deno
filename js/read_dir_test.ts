// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";
import { FileInfo } from "deno";

function assertSameContent(files: FileInfo[]) {
  let counter = 0;

  for (const file of files) {
    if (file.name === "subdir") {
      assert(file.isDirectory());
      counter++;
    }

    if (file.name === "002_hello.ts") {
      assertEqual(file.path, `tests/${file.name}`);
      assertEqual(file.mode!, deno.statSync(`tests/${file.name}`).mode!);
      counter++;
    }
  }

  assertEqual(counter, 2);
}

testPerm({ write: true }, function readDirSyncSuccess() {
  const files = deno.readDirSync("tests/");
  assertSameContent(files);
});

test(function readDirSyncNotDir() {
  let caughtError = false;
  let src;

  try {
    src = deno.readDirSync("package.json");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.Other);
  }
  assert(caughtError);
  assertEqual(src, undefined);
});

test(function readDirSyncNotFound() {
  let caughtError = false;
  let src;

  try {
    src = deno.readDirSync("bad_dir_name");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.NotFound);
  }
  assert(caughtError);
  assertEqual(src, undefined);
});

testPerm({ write: true }, async function readDirSuccess() {
  const files = await deno.readDir("tests/");
  assertSameContent(files);
});
