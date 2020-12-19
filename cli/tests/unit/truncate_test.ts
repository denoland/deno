// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  unitTest,
} from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function ftruncateSyncSuccess(): void {
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

unitTest(
  { perms: { read: true, write: true } },
  async function ftruncateSuccess(): Promise<void> {
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

unitTest(
  { perms: { read: true, write: true } },
  function truncateSyncSuccess(): void {
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

unitTest(
  { perms: { read: true, write: true } },
  async function truncateSuccess(): Promise<void> {
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

unitTest({ perms: { write: false } }, function truncateSyncPerm(): void {
  assertThrows(() => {
    Deno.truncateSync("/test_truncateSyncPermission.txt");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { write: false } }, async function truncatePerm(): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    await Deno.truncate("/test_truncatePermission.txt");
  }, Deno.errors.PermissionDenied);
});
