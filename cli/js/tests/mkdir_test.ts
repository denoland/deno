// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncSuccess(): void {
    const path = Deno.makeTempDirSync() + "/dir";
    Deno.mkdirSync(path);
    const pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory());
  }
);

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncMode(): void {
    const path = Deno.makeTempDirSync() + "/dir";
    Deno.mkdirSync(path, { mode: 0o737 });
    const pathInfo = Deno.statSync(path);
    if (Deno.build.os !== "win") {
      assertEquals(pathInfo.mode! & 0o777, 0o737 & ~Deno.umask());
    }
  }
);

unitTest({ perms: { write: false } }, function mkdirSyncPerm(): void {
  let err;
  try {
    Deno.mkdirSync("/baddir");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

unitTest(
  { perms: { read: true, write: true } },
  async function mkdirSuccess(): Promise<void> {
    const path = Deno.makeTempDirSync() + "/dir";
    await Deno.mkdir(path);
    const pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory());
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function mkdirMode(): Promise<void> {
    const path = Deno.makeTempDirSync() + "/dir";
    await Deno.mkdir(path, { mode: 0o737 });
    const pathInfo = Deno.statSync(path);
    if (Deno.build.os !== "win") {
      assertEquals(pathInfo.mode! & 0o777, 0o737 & ~Deno.umask());
    }
  }
);

unitTest({ perms: { write: true } }, function mkdirErrIfExists(): void {
  let err;
  try {
    Deno.mkdirSync(".");
  } catch (e) {
    err = e;
  }
  assert(err instanceof Deno.errors.AlreadyExists);
});

unitTest(
  { perms: { read: true, write: true } },
  function mkdirSyncRecursive(): void {
    const path = Deno.makeTempDirSync() + "/nested/directory";
    Deno.mkdirSync(path, { recursive: true });
    const pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory());
  }
);

unitTest(
  { perms: { read: true, write: true } },
  async function mkdirRecursive(): Promise<void> {
    const path = Deno.makeTempDirSync() + "/nested/directory";
    await Deno.mkdir(path, { recursive: true });
    const pathInfo = Deno.statSync(path);
    assert(pathInfo.isDirectory());
  }
);
