// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { testPerm, assertEquals, assert } from "./test_util.ts";

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

testPerm({ read: true, write: true }, function truncateSyncSuccess(): void {
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
});

testPerm({ read: true, write: true }, async function truncateSuccess(): Promise<
  void
> {
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
});

testPerm({ write: false }, function truncateSyncPerm(): void {
  let err;
  try {
    Deno.mkdirSync("/test_truncateSyncPermission.txt");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ write: false }, async function truncatePerm(): Promise<void> {
  let err;
  try {
    await Deno.mkdir("/test_truncatePermission.txt");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: true }, function ftruncateSyncSuccess(): void {
  const enc = new TextEncoder();
  const d = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test_truncateSync.txt";
  Deno.writeFileSync(filename, d);
  const f1 = Deno.openSync(filename, "r+");
  f1.truncateSync(20);
  f1.close();
  let data = readDataSync(filename);
  assertEquals(data.length, 20);
  const f2 = Deno.openSync(filename, "r+");
  f2.truncateSync(5);
  f2.close();
  data = readDataSync(filename);
  assertEquals(data.length, 5);
  const f3 = Deno.openSync(filename, "r+");
  f3.truncateSync(-5);
  f3.close();
  data = readDataSync(filename);
  assertEquals(data.length, 0);
  Deno.removeSync(filename);
});

testPerm(
  { read: true, write: true },
  async function ftruncateSuccess(): Promise<void> {
    const enc = new TextEncoder();
    const d = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test_truncate.txt";
    await Deno.writeFile(filename, d);
    const f1 = await Deno.open(filename, "r+");
    await f1.truncate(20);
    f1.close();
    let data = await readData(filename);
    assertEquals(data.length, 20);
    const f2 = await Deno.open(filename, "r+");
    await f2.truncate(5);
    f2.close();
    data = await readData(filename);
    assertEquals(data.length, 5);
    const f3 = await Deno.open(filename, "r+");
    await f3.truncate(-5);
    f3.close();
    data = await readData(filename);
    assertEquals(data.length, 0);
    await Deno.remove(filename);
  }
);

testPerm({ read: true, write: false }, function ftruncateSyncPerm(): void {
  let err;
  let caughtError = false;
  const f = Deno.openSync("README.md", "r");
  try {
    f.truncateSync(0);
  } catch (e) {
    caughtError = true;
    err = e;
  }
  f.close();
  // throw if we lack --write permissions
  assert(caughtError);
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: false }, async function ftruncatePerm(): Promise<
  void
> {
  let err;
  let caughtError = false;
  const f = await Deno.open("README.md", "r");
  try {
    await f.truncate(0);
  } catch (e) {
    caughtError = true;
    err = e;
  }
  f.close();
  // throw if we lack --write permissions
  assert(caughtError);
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: true }, function ftruncateSyncPerm2(): void {
  let err;
  let caughtError = false;
  const filename = Deno.makeTempDirSync() + "/test_truncateSync.txt";
  const f0 = Deno.openSync(filename, "w");
  f0.close();
  const f = Deno.openSync(filename, "r");
  try {
    f.truncateSync(0);
  } catch (e) {
    caughtError = true;
    err = e;
  }
  f.close();
  // fd is not opened for writing
  assert(caughtError);
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: true }, async function ftruncatePerm2(): Promise<
  void
> {
  let err;
  let caughtError = false;
  const filename = (await Deno.makeTempDir()) + "/test_truncate.txt";
  const f0 = await Deno.open(filename, "w");
  f0.close();
  const f = await Deno.open(filename, "r");
  try {
    await f.truncate(0);
  } catch (e) {
    caughtError = true;
    err = e;
  }
  f.close();
  // fd is not opened for writing
  assert(caughtError);
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});
