// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
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

Deno.test("readDirSyncSuccess", function (): void {
  const files = [...Deno.readDirSync("cli/tests/")];
  assertSameContent(files);
});

Deno.test("readDirSyncWithUrl", function (): void {
  const files = [...Deno.readDirSync(pathToAbsoluteFileUrl("cli/tests"))];
  assertSameContent(files);
});

Deno.test("readDirSyncNotDir", function (): void {
  assertThrows(() => {
    Deno.readDirSync("cli/tests/fixture.json");
  }, Error);
});

Deno.test("readDirSyncNotFound", function (): void {
  assertThrows(() => {
    Deno.readDirSync("bad_dir_name");
  }, Deno.errors.NotFound);
});

Deno.test("readDirSuccess", async function (): Promise<
  void
> {
  const files = [];
  for await (const dirEntry of Deno.readDir("cli/tests/")) {
    files.push(dirEntry);
  }
  assertSameContent(files);
});

Deno.test("readDirWithUrl", async function (): Promise<
  void
> {
  const files = [];
  for await (
    const dirEntry of Deno.readDir(pathToAbsoluteFileUrl("cli/tests"))
  ) {
    files.push(dirEntry);
  }
  assertSameContent(files);
});

Deno.test({
  name: "readDirDevFd",
  ignore: Deno.build.os == "windows",
  async fn(): Promise<
    void
  > {
    for await (const _ of Deno.readDir("/dev/fd")) {
      // We don't actually care whats in here; just that we don't panic on non regular entries
    }
  },
});

Deno.test({
  name: "readDirDevFdSync",
  ignore: Deno.build.os == "windows",
  fn(): void {
    for (const _ of Deno.readDirSync("/dev/fd")) {
      // We don't actually care whats in here; just that we don't panic on non regular file entries
    }
  },
});

Deno.test("readDirPerm", async function (): Promise<
  void
> {
  await Deno.permissions.revoke({ name: "read" });

  await assertThrowsAsync(async () => {
    await Deno.readDir("tests/")[Symbol.asyncIterator]().next();
  }, Deno.errors.PermissionDenied);
});

Deno.test("readDirSyncPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  assertThrows(() => {
    Deno.readDirSync("tests/");
  }, Deno.errors.PermissionDenied);
});
