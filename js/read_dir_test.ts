// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

type FileInfo = Deno.FileInfo;

function assertSameContent(files: FileInfo[]): void {
  let counter = 0;

  for (const file of files) {
    if (file.name === "subdir") {
      assert(file.isDirectory());
      counter++;
    }

    if (file.name === "002_hello.ts") {
      assertEquals(file.path, `tests/${file.name}`);
      assertEquals(file.mode!, Deno.statSync(`tests/${file.name}`).mode!);
      counter++;
    }
  }

  assertEquals(counter, 2);
}

testPerm({ read: true }, function readDirSyncSuccess(): void {
  const files = Deno.readDirSync("tests/");
  assertSameContent(files);
});

testPerm({ read: false }, function readDirSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.readDirSync("tests/");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true }, function readDirSyncNotDir(): void {
  let caughtError = false;
  let src;

  try {
    src = Deno.readDirSync("package.json");
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.Other);
  }
  assert(caughtError);
  assertEquals(src, undefined);
});

testPerm({ read: true }, function readDirSyncNotFound(): void {
  let caughtError = false;
  let src;

  try {
    src = Deno.readDirSync("bad_dir_name");
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.NotFound);
  }
  assert(caughtError);
  assertEquals(src, undefined);
});

testPerm({ read: true }, async function readDirSuccess(): Promise<void> {
  const files = await Deno.readDir("tests/");
  assertSameContent(files);
});

testPerm({ read: false }, async function readDirPerm(): Promise<void> {
  let caughtError = false;
  try {
    await Deno.readDir("tests/");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});
