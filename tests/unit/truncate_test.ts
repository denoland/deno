// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects, assertThrows } from "./test_util.ts";

Deno.test(
  { permissions: { read: true, write: true } },
  function ftruncateSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_ftruncateSync.txt";
    using file = Deno.openSync(filename, {
      create: true,
      read: true,
      write: true,
    });

    file.truncateSync(20);
    assertEquals(Deno.readFileSync(filename).byteLength, 20);
    file.truncateSync(5);
    assertEquals(Deno.readFileSync(filename).byteLength, 5);
    file.truncateSync(-5);
    assertEquals(Deno.readFileSync(filename).byteLength, 0);

    Deno.removeSync(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function ftruncateSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_ftruncate.txt";
    using file = await Deno.open(filename, {
      create: true,
      read: true,
      write: true,
    });

    await file.truncate(20);
    assertEquals((await Deno.readFile(filename)).byteLength, 20);
    await file.truncate(5);
    assertEquals((await Deno.readFile(filename)).byteLength, 5);
    await file.truncate(-5);
    assertEquals((await Deno.readFile(filename)).byteLength, 0);

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
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { write: false } }, async function truncatePerm() {
  await assertRejects(async () => {
    await Deno.truncate("/test_truncatePermission.txt");
  }, Deno.errors.NotCapable);
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
