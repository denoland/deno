// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals, assert } from "./test_util.ts";

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
  }
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
  }
);

unitTest({ perms: { write: false } }, function truncateSyncPerm(): void {
  let err;
  try {
    Deno.truncateSync("/test_truncateSyncPermission.txt");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

unitTest({ perms: { write: false } }, async function truncatePerm(): Promise<
  void
> {
  let err;
  try {
    await Deno.truncate("/test_truncatePermission.txt");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});
