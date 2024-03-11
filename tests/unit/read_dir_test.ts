// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

function assertSameContent(files: Deno.DirEntry[]) {
  let counter = 0;

  for (const entry of files) {
    if (entry.name === "subdir") {
      assert(entry.isDirectory);
      counter++;
    }
  }

  assertEquals(counter, 1);
}

Deno.test({ permissions: { read: true } }, function readDirSyncSuccess() {
  const files = [...Deno.readDirSync("tests/testdata")];
  assertSameContent(files);
});

Deno.test({ permissions: { read: true } }, function readDirSyncWithUrl() {
  const files = [
    ...Deno.readDirSync(pathToAbsoluteFileUrl("tests/testdata")),
  ];
  assertSameContent(files);
});

Deno.test({ permissions: { read: false } }, function readDirSyncPerm() {
  assertThrows(() => {
    Deno.readDirSync("tests/");
  }, Deno.errors.PermissionDenied);
});

Deno.test({ permissions: { read: true } }, function readDirSyncNotDir() {
  assertThrows(
    () => {
      Deno.readDirSync("tests/testdata/assets/fixture.json");
    },
    Error,
    `readdir 'tests/testdata/assets/fixture.json'`,
  );
});

Deno.test({ permissions: { read: true } }, function readDirSyncNotFound() {
  assertThrows(
    () => {
      Deno.readDirSync("bad_dir_name");
    },
    Deno.errors.NotFound,
    `readdir 'bad_dir_name'`,
  );
});

Deno.test({ permissions: { read: true } }, async function readDirSuccess() {
  const files = [];
  for await (const dirEntry of Deno.readDir("tests/testdata")) {
    files.push(dirEntry);
  }
  assertSameContent(files);
});

Deno.test({ permissions: { read: true } }, async function readDirWithUrl() {
  const files = [];
  for await (
    const dirEntry of Deno.readDir(pathToAbsoluteFileUrl("tests/testdata"))
  ) {
    files.push(dirEntry);
  }
  assertSameContent(files);
});

Deno.test({ permissions: { read: false } }, async function readDirPerm() {
  await assertRejects(async () => {
    await Deno.readDir("tests/")[Symbol.asyncIterator]().next();
  }, Deno.errors.PermissionDenied);
});

Deno.test({ permissions: { read: true } }, async function readDirNotFound() {
  await assertRejects(
    async () => {
      await Deno.readDir("bad_dir_name")[Symbol.asyncIterator]().next();
    },
    Deno.errors.NotFound,
    `readdir 'bad_dir_name'`,
  );
});
