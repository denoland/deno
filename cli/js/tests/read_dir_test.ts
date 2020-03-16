// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

type FileInfo = Deno.FileInfo;

function assertSameContent(files: FileInfo[]): void {
  let counter = 0;

  for (const file of files) {
    if (file.name === "subdir") {
      assert(file.isDirectory());
      counter++;
    }

    if (file.name === "002_hello.ts") {
      assertEquals(file.mode!, Deno.statSync(`cli/tests/${file.name}`).mode!);
      counter++;
    }
  }

  assertEquals(counter, 2);
}

unitTest({ perms: { read: true } }, function readdirSyncSuccess(): void {
  const files = Deno.readdirSync("cli/tests/");
  assertSameContent(files);
});

unitTest({ perms: { read: false } }, function readdirSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.readdirSync("tests/");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function readdirSyncNotDir(): void {
  let caughtError = false;
  let src;

  try {
    src = Deno.readdirSync("cli/tests/fixture.json");
  } catch (err) {
    caughtError = true;
    assert(err instanceof Error);
  }
  assert(caughtError);
  assertEquals(src, undefined);
});

unitTest({ perms: { read: true } }, function readdirSyncNotFound(): void {
  let caughtError = false;
  let src;

  try {
    src = Deno.readdirSync("bad_dir_name");
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
  assertEquals(src, undefined);
});

unitTest({ perms: { read: true } }, async function readdirSuccess(): Promise<
  void
> {
  const files = await Deno.readdir("cli/tests/");
  assertSameContent(files);
});

unitTest({ perms: { read: false } }, async function readdirPerm(): Promise<
  void
> {
  let caughtError = false;
  try {
    await Deno.readdir("tests/");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});
