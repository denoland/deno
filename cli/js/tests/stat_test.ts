// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";

// TODO Add tests for modified, accessed, and created fields once there is a way
// to create temp files.
unitTest({ perms: { read: true } }, function statSyncSuccess(): void {
  const packageInfo = Deno.statSync("README.md");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const modulesInfo = Deno.statSync("cli/tests/symlink_to_subdir");
  assert(modulesInfo.isDirectory());
  assert(!modulesInfo.isSymlink());

  const testsInfo = Deno.statSync("cli/tests");
  assert(testsInfo.isDirectory());
  assert(!testsInfo.isSymlink());
});

unitTest({ perms: { read: false } }, function statSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.statSync("README.md");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function statSyncNotFound(): void {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = Deno.statSync("bad_file_name");
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.NotFound);
  }

  assert(caughtError);
  assertEquals(badInfo, undefined);
});

unitTest({ perms: { read: true } }, function lstatSyncSuccess(): void {
  const packageInfo = Deno.lstatSync("README.md");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const modulesInfo = Deno.lstatSync("cli/tests/symlink_to_subdir");
  assert(!modulesInfo.isDirectory());
  assert(modulesInfo.isSymlink());

  const coreInfo = Deno.lstatSync("core");
  assert(coreInfo.isDirectory());
  assert(!coreInfo.isSymlink());
});

unitTest({ perms: { read: false } }, function lstatSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.lstatSync("README.md");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function lstatSyncNotFound(): void {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = Deno.lstatSync("bad_file_name");
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.NotFound);
  }

  assert(caughtError);
  assertEquals(badInfo, undefined);
});

unitTest({ perms: { read: true } }, async function statSuccess(): Promise<
  void
> {
  const packageInfo = await Deno.stat("README.md");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const modulesInfo = await Deno.stat("cli/tests/symlink_to_subdir");
  assert(modulesInfo.isDirectory());
  assert(!modulesInfo.isSymlink());

  const testsInfo = await Deno.stat("cli/tests");
  assert(testsInfo.isDirectory());
  assert(!testsInfo.isSymlink());
});

unitTest({ perms: { read: false } }, async function statPerm(): Promise<void> {
  let caughtError = false;
  try {
    await Deno.stat("README.md");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, async function statNotFound(): Promise<
  void
> {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = await Deno.stat("bad_file_name");
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.NotFound);
  }

  assert(caughtError);
  assertEquals(badInfo, undefined);
});

unitTest({ perms: { read: true } }, async function lstatSuccess(): Promise<
  void
> {
  const packageInfo = await Deno.lstat("README.md");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const modulesInfo = await Deno.lstat("cli/tests/symlink_to_subdir");
  assert(!modulesInfo.isDirectory());
  assert(modulesInfo.isSymlink());

  const coreInfo = await Deno.lstat("core");
  assert(coreInfo.isDirectory());
  assert(!coreInfo.isSymlink());
});

unitTest({ perms: { read: false } }, async function lstatPerm(): Promise<void> {
  let caughtError = false;
  try {
    await Deno.lstat("README.md");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, async function lstatNotFound(): Promise<
  void
> {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = await Deno.lstat("bad_file_name");
  } catch (err) {
    caughtError = true;
    assert(err instanceof Deno.errors.NotFound);
  }

  assert(caughtError);
  assertEquals(badInfo, undefined);
});

unitTest(
  { ignore: Deno.build.os !== "win", perms: { read: true, write: true } },
  function statNoUnixFields(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const s = Deno.statSync(filename);
    assert(s.dev === null);
    assert(s.ino === null);
    assert(s.mode === null);
    assert(s.nlink === null);
    assert(s.uid === null);
    assert(s.gid === null);
    assert(s.rdev === null);
    assert(s.blksize === null);
    assert(s.blocks === null);
  }
);

unitTest(
  { ignore: Deno.build.os === "win", perms: { read: true, write: true } },
  function statUnixFields(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    const filename2 = tempDir + "/test2.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    // Create a link
    Deno.linkSync(filename, filename2);
    const s = Deno.statSync(filename);
    assert(s.dev !== null);
    assert(s.ino !== null);
    assertEquals(s.mode! & 0o666, 0o666);
    assertEquals(s.nlink, 2);
    assert(s.uid !== null);
    assert(s.gid !== null);
    assert(s.rdev !== null);
    assert(s.blksize !== null);
    assert(s.blocks !== null);
  }
);
