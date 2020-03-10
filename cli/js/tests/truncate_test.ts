// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals, assert } from "./test_util.ts";

function readDataSync(name: string): string {
  const data = Deno.readFileSync(name);
  const decoder = new TextDecoder("utf-8");
  const text = decoder.decode(data);
  return text;
}

async function readData(name: string): Promise<string> {
  const data = await Deno.readFile(name);
  const decoder = new TextDecoder("utf-8");
  const text = decoder.decode(data);
  return text;
}

unitTest(
  { perms: { read: true, write: true } },
  function truncateSyncSuccess(): void {
    const enc = new TextEncoder();
    const d = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test_truncateSync.txt";
    Deno.writeFileSync(filename, d);
    Deno.truncateSync(filename, 20);
    let data = readDataSync(filename);
    assertEquals(data.length, 20);
    Deno.truncateSync(filename, 5);
    data = readDataSync(filename);
    assertEquals(data.length, 5);
    Deno.truncateSync(filename, -5);
    data = readDataSync(filename);
    assertEquals(data.length, 0);
    Deno.removeSync(filename);
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function truncateSuccess(): Promise<void> {
    const enc = new TextEncoder();
    const d = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test_truncate.txt";
    await Deno.writeFile(filename, d);
    await Deno.truncate(filename, 20);
    let data = await readData(filename);
    assertEquals(data.length, 20);
    await Deno.truncate(filename, 5);
    data = await readData(filename);
    assertEquals(data.length, 5);
    await Deno.truncate(filename, -5);
    data = await readData(filename);
    assertEquals(data.length, 0);
    await Deno.remove(filename);
  }
);

unitTest(
  {
    skip: Deno.build.os === "win",
    perms: { read: true, write: true }
  },
  function truncateSyncMode(): void {
    const path = Deno.makeTempDirSync() + "/test_truncateSync.txt";
    Deno.truncateSync(path, 20, { mode: 0o626 });
    const pathInfo = Deno.statSync(path);
    // assertEquals(pathInfo.mode!, 0o626 & ~Deno.umask());
    assertEquals(pathInfo.mode! & 0o777, 0o604); // assume umask 0o022
  }
);

unitTest(
  {
    skip: Deno.build.os === "win",
    perms: { read: true, write: true }
  },
  async function truncateMode(): Promise<void> {
    const path = (await Deno.makeTempDir()) + "/test_truncate.txt";
    await Deno.truncate(path, 20, { mode: 0o626 });
    const pathInfo = Deno.statSync(path);
    // assertEquals(pathInfo.mode!, 0o626 & ~Deno.umask());
    assertEquals(pathInfo.mode! & 0o777, 0o604); // assume umask 0o022
  }
);

unitTest({ perms: { write: false } }, function truncateSyncPerm(): void {
  let err;
  try {
    Deno.mkdirSync("/test_truncateSyncPermission.txt");
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
    await Deno.mkdir("/test_truncatePermission.txt");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});
