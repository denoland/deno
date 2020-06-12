// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  unitTest,
  assert,
  assertEquals,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

function assertSameContent(files: Deno.DirEntry[]): void {
  let counter = 0;

  for (const entry of files) {
    if (entry.name === "subdir") {
      assert(entry.isDirectory);
      counter++;
    }
  }

  assertEquals(counter, 1);
}

unitTest({ perms: { read: true } }, function readDirSyncSuccess(): void {
  const files = [...Deno.readDirSync("cli/tests/")];
  assertSameContent(files);
});

unitTest({ perms: { read: true } }, function readDirSyncWithUrl(): void {
  const files = [...Deno.readDirSync(pathToAbsoluteFileUrl("cli/tests"))];
  assertSameContent(files);
});

unitTest({ perms: { read: false } }, function readDirSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.readDirSync("tests/");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function readDirSyncNotDir(): void {
  let caughtError = false;
  let src;

  try {
    src = Deno.readDirSync("cli/tests/fixture.json");
  } catch (err) {
    caughtError = true;
    assert(err instanceof Error);
  }
  assert(caughtError);
  assertEquals(src, undefined);
});

unitTest({ perms: { read: true } }, function readDirSyncNotFound(): void {
  let caughtError = false;
  let src;

  try {
    src = Deno.readDirSync("bad_dir_name");
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
  assertEquals(src, undefined);
});

unitTest({ perms: { read: true } }, async function readDirSuccess(): Promise<
  void
> {
  const files = [];
  for await (const dirEntry of Deno.readDir("cli/tests/")) {
    files.push(dirEntry);
  }
  assertSameContent(files);
});

unitTest({ perms: { read: true } }, async function readDirWithUrl(): Promise<
  void
> {
  const files = [];
  for await (const dirEntry of Deno.readDir(
    pathToAbsoluteFileUrl("cli/tests")
  )) {
    files.push(dirEntry);
  }
  assertSameContent(files);
});

unitTest({ perms: { read: false } }, async function readDirPerm(): Promise<
  void
> {
  let caughtError = false;
  try {
    await Deno.readDir("tests/")[Symbol.asyncIterator]().next();
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});
