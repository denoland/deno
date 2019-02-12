// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";

type FileInfo = Deno.FileInfo;

function assertSameContent(files: FileInfo[]) {
  let counter = 0;

  for (const file of files) {
    if (file.name === "subdir") {
      assert(file.isDirectory());
      counter++;
    }

    if (file.name === "002_hello.ts") {
      assertEqual(file.path, `tests/${file.name}`);
      assertEqual(file.mode!, Deno.statSync(`tests/${file.name}`).mode!);
      counter++;
    }
  }

  assertEqual(counter, 2);
}

testPerm({ read: true }, function readDirSyncSuccess() {
  const files = Deno.readDirSync("tests/");
  assertSameContent(files);
});

testPerm({ read: false }, function readDirSyncPerm() {
  let caughtError = false;
  try {
    const files = Deno.readDirSync("tests/");
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true }, function readDirSyncNotDir() {
  let caughtError = false;
  let src;

  try {
    src = Deno.readDirSync("package.json");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, Deno.ErrorKind.Other);
  }
  assert(caughtError);
  assertEqual(src, undefined);
});

testPerm({ read: true }, function readDirSyncNotFound() {
  let caughtError = false;
  let src;

  try {
    src = Deno.readDirSync("bad_dir_name");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, Deno.ErrorKind.NotFound);
  }
  assert(caughtError);
  assertEqual(src, undefined);
});

testPerm({ read: true }, async function readDirSuccess() {
  const files = await Deno.readDir("tests/");
  assertSameContent(files);
});

testPerm({ read: false }, async function readDirPerm() {
  let caughtError = false;
  try {
    const files = await Deno.readDir("tests/");
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});
