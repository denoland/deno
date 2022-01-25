// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects, assertThrows } from "./test_util.ts";

Deno.test(
  { permissions: { read: true, write: true } },
  function ftruncateSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_ftruncateSync.txt";
    const file = Deno.openSync(filename, {
      create: true,
      read: true,
      write: true,
    });

    Deno.ftruncateSync(file.rid, 20);
    assertEquals(Deno.readFileSync(filename).byteLength, 20);
    Deno.ftruncateSync(file.rid, 5);
    assertEquals(Deno.readFileSync(filename).byteLength, 5);
    Deno.ftruncateSync(file.rid, -5);
    assertEquals(Deno.readFileSync(filename).byteLength, 0);

    Deno.close(file.rid);
    Deno.removeSync(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function ftruncateSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_ftruncate.txt";
    const file = await Deno.open(filename, {
      create: true,
      read: true,
      write: true,
    });

    await Deno.ftruncate(file.rid, 20);
    assertEquals((await Deno.readFile(filename)).byteLength, 20);
    await Deno.ftruncate(file.rid, 5);
    assertEquals((await Deno.readFile(filename)).byteLength, 5);
    await Deno.ftruncate(file.rid, -5);
    assertEquals((await Deno.readFile(filename)).byteLength, 0);

    Deno.close(file.rid);
    await Deno.remove(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function truncateSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_truncateSync.txt";
    Deno.writeFileSync(filename, new Uint8Array(5));
    Deno.truncateSync(filename, 20);
    assertEquals(Deno.readFileSync(filename).byteLength, 20);
    Deno.truncateSync(filename, 5);
    assertEquals(Deno.readFileSync(filename).byteLength, 5);
    Deno.truncateSync(filename, -5);
    assertEquals(Deno.readFileSync(filename).byteLength, 0);
    Deno.removeSync(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function truncateSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_truncate.txt";
    await Deno.writeFile(filename, new Uint8Array(5));
    await Deno.truncate(filename, 20);
    assertEquals((await Deno.readFile(filename)).byteLength, 20);
    await Deno.truncate(filename, 5);
    assertEquals((await Deno.readFile(filename)).byteLength, 5);
    await Deno.truncate(filename, -5);
    assertEquals((await Deno.readFile(filename)).byteLength, 0);
    await Deno.remove(filename);
  },
);

Deno.test({ permissions: { write: false } }, function truncateSyncPerm() {
  assertThrows(() => {
    Deno.truncateSync("/test_truncateSyncPermission.txt");
  }, Deno.errors.PermissionDenied);
});

Deno.test({ permissions: { write: false } }, async function truncatePerm() {
  await assertRejects(async () => {
    await Deno.truncate("/test_truncatePermission.txt");
  }, Deno.errors.PermissionDenied);
});

Deno.test(
  { permissions: { read: true, write: true } },
  function truncateSyncNotFound() {
    const filename = "/badfile.txt";
    assertThrows(
      () => {
        Deno.truncateSync(filename);
      },
      Deno.errors.NotFound,
      `truncate '${filename}'`,
    );
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function truncateSyncNotFound() {
    const filename = "/badfile.txt";
    await assertRejects(
      async () => {
        await Deno.truncate(filename);
      },
      Deno.errors.NotFound,
      `truncate '${filename}'`,
    );
  },
);
