// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

unitTest({ perms: { read: true } }, function fstatSyncSuccess(): void {
  const file = Deno.openSync("README.md");
  const fileInfo = Deno.fstatSync(file.rid);
  assert(fileInfo.isFile);
  assert(!fileInfo.isSymlink);
  assert(!fileInfo.isDirectory);
  assert(fileInfo.size);
  assert(fileInfo.atime);
  assert(fileInfo.mtime);
  // The `birthtime` field is not available on Linux before kernel version 4.11.
  assert(fileInfo.birthtime || Deno.build.os === "linux");

  Deno.close(file.rid);
});

unitTest({ perms: { read: true } }, async function fstatSuccess(): Promise<
  void
> {
  const file = await Deno.open("README.md");
  const fileInfo = await Deno.fstat(file.rid);
  assert(fileInfo.isFile);
  assert(!fileInfo.isSymlink);
  assert(!fileInfo.isDirectory);
  assert(fileInfo.size);
  assert(fileInfo.atime);
  assert(fileInfo.mtime);
  // The `birthtime` field is not available on Linux before kernel version 4.11.
  assert(fileInfo.birthtime || Deno.build.os === "linux");

  Deno.close(file.rid);
});

unitTest(
  { perms: { read: true, write: true } },
  function statSyncSuccess(): void {
    const packageInfo = Deno.statSync("README.md");
    assert(packageInfo.isFile);
    assert(!packageInfo.isSymlink);

    const modulesInfo = Deno.statSync("cli/tests/symlink_to_subdir");
    assert(modulesInfo.isDirectory);
    assert(!modulesInfo.isSymlink);

    const testsInfo = Deno.statSync("cli/tests");
    assert(testsInfo.isDirectory);
    assert(!testsInfo.isSymlink);

    const tempFile = Deno.makeTempFileSync();
    const tempInfo = Deno.statSync(tempFile);
    let now = Date.now();
    assert(tempInfo.atime !== null && now - tempInfo.atime.valueOf() < 1000);
    assert(tempInfo.mtime !== null && now - tempInfo.mtime.valueOf() < 1000);
    assert(
      tempInfo.birthtime === null || now - tempInfo.birthtime.valueOf() < 1000,
    );

    const packageInfoByUrl = Deno.statSync(pathToAbsoluteFileUrl("README.md"));
    assert(packageInfoByUrl.isFile);
    assert(!packageInfoByUrl.isSymlink);

    const modulesInfoByUrl = Deno.statSync(
      pathToAbsoluteFileUrl("cli/tests/symlink_to_subdir"),
    );
    assert(modulesInfoByUrl.isDirectory);
    assert(!modulesInfoByUrl.isSymlink);

    const testsInfoByUrl = Deno.statSync(pathToAbsoluteFileUrl("cli/tests"));
    assert(testsInfoByUrl.isDirectory);
    assert(!testsInfoByUrl.isSymlink);

    const tempFileForUrl = Deno.makeTempFileSync();
    const tempInfoByUrl = Deno.statSync(
      new URL(
        `file://${Deno.build.os === "windows" ? "/" : ""}${tempFileForUrl}`,
      ),
    );
    now = Date.now();
    assert(
      tempInfoByUrl.atime !== null &&
        now - tempInfoByUrl.atime.valueOf() < 1000,
    );
    assert(
      tempInfoByUrl.mtime !== null &&
        now - tempInfoByUrl.mtime.valueOf() < 1000,
    );
    assert(
      tempInfoByUrl.birthtime === null ||
        now - tempInfoByUrl.birthtime.valueOf() < 1000,
    );

    Deno.removeSync(tempFile, { recursive: true });
    Deno.removeSync(tempFileForUrl, { recursive: true });
  },
);

unitTest({ perms: { read: false } }, function statSyncPerm(): void {
  assertThrows(() => {
    Deno.statSync("README.md");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function statSyncNotFound(): void {
  assertThrows(() => {
    Deno.statSync("bad_file_name");
  }, Deno.errors.NotFound);
});

unitTest({ perms: { read: true } }, function lstatSyncSuccess(): void {
  const packageInfo = Deno.lstatSync("README.md");
  assert(packageInfo.isFile);
  assert(!packageInfo.isSymlink);

  const packageInfoByUrl = Deno.lstatSync(pathToAbsoluteFileUrl("README.md"));
  assert(packageInfoByUrl.isFile);
  assert(!packageInfoByUrl.isSymlink);

  const modulesInfo = Deno.lstatSync("cli/tests/symlink_to_subdir");
  assert(!modulesInfo.isDirectory);
  assert(modulesInfo.isSymlink);

  const modulesInfoByUrl = Deno.lstatSync(
    pathToAbsoluteFileUrl("cli/tests/symlink_to_subdir"),
  );
  assert(!modulesInfoByUrl.isDirectory);
  assert(modulesInfoByUrl.isSymlink);

  const coreInfo = Deno.lstatSync("core");
  assert(coreInfo.isDirectory);
  assert(!coreInfo.isSymlink);

  const coreInfoByUrl = Deno.lstatSync(pathToAbsoluteFileUrl("core"));
  assert(coreInfoByUrl.isDirectory);
  assert(!coreInfoByUrl.isSymlink);
});

unitTest({ perms: { read: false } }, function lstatSyncPerm(): void {
  assertThrows(() => {
    Deno.lstatSync("README.md");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function lstatSyncNotFound(): void {
  assertThrows(() => {
    Deno.lstatSync("bad_file_name");
  }, Deno.errors.NotFound);
});

unitTest(
  { perms: { read: true, write: true } },
  async function statSuccess(): Promise<void> {
    const packageInfo = await Deno.stat("README.md");
    assert(packageInfo.isFile);
    assert(!packageInfo.isSymlink);

    const packageInfoByUrl = await Deno.stat(
      pathToAbsoluteFileUrl("README.md"),
    );
    assert(packageInfoByUrl.isFile);
    assert(!packageInfoByUrl.isSymlink);

    const modulesInfo = await Deno.stat("cli/tests/symlink_to_subdir");
    assert(modulesInfo.isDirectory);
    assert(!modulesInfo.isSymlink);

    const modulesInfoByUrl = await Deno.stat(
      pathToAbsoluteFileUrl("cli/tests/symlink_to_subdir"),
    );
    assert(modulesInfoByUrl.isDirectory);
    assert(!modulesInfoByUrl.isSymlink);

    const testsInfo = await Deno.stat("cli/tests");
    assert(testsInfo.isDirectory);
    assert(!testsInfo.isSymlink);

    const testsInfoByUrl = await Deno.stat(pathToAbsoluteFileUrl("cli/tests"));
    assert(testsInfoByUrl.isDirectory);
    assert(!testsInfoByUrl.isSymlink);

    const tempFile = await Deno.makeTempFile();
    const tempInfo = await Deno.stat(tempFile);
    let now = Date.now();
    assert(tempInfo.atime !== null && now - tempInfo.atime.valueOf() < 1000);
    assert(tempInfo.mtime !== null && now - tempInfo.mtime.valueOf() < 1000);

    assert(
      tempInfo.birthtime === null || now - tempInfo.birthtime.valueOf() < 1000,
    );

    const tempFileForUrl = await Deno.makeTempFile();
    const tempInfoByUrl = await Deno.stat(
      new URL(
        `file://${Deno.build.os === "windows" ? "/" : ""}${tempFileForUrl}`,
      ),
    );
    now = Date.now();
    assert(
      tempInfoByUrl.atime !== null &&
        now - tempInfoByUrl.atime.valueOf() < 1000,
    );
    assert(
      tempInfoByUrl.mtime !== null &&
        now - tempInfoByUrl.mtime.valueOf() < 1000,
    );
    assert(
      tempInfoByUrl.birthtime === null ||
        now - tempInfoByUrl.birthtime.valueOf() < 1000,
    );

    Deno.removeSync(tempFile, { recursive: true });
    Deno.removeSync(tempFileForUrl, { recursive: true });
  },
);

unitTest({ perms: { read: false } }, async function statPerm(): Promise<void> {
  await assertThrowsAsync(async () => {
    await Deno.stat("README.md");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, async function statNotFound(): Promise<
  void
> {
  await assertThrowsAsync(
    async (): Promise<void> => {
      await Deno.stat("bad_file_name"), Deno.errors.NotFound;
    },
  );
});

unitTest({ perms: { read: true } }, async function lstatSuccess(): Promise<
  void
> {
  const packageInfo = await Deno.lstat("README.md");
  assert(packageInfo.isFile);
  assert(!packageInfo.isSymlink);

  const packageInfoByUrl = await Deno.lstat(pathToAbsoluteFileUrl("README.md"));
  assert(packageInfoByUrl.isFile);
  assert(!packageInfoByUrl.isSymlink);

  const modulesInfo = await Deno.lstat("cli/tests/symlink_to_subdir");
  assert(!modulesInfo.isDirectory);
  assert(modulesInfo.isSymlink);

  const modulesInfoByUrl = await Deno.lstat(
    pathToAbsoluteFileUrl("cli/tests/symlink_to_subdir"),
  );
  assert(!modulesInfoByUrl.isDirectory);
  assert(modulesInfoByUrl.isSymlink);

  const coreInfo = await Deno.lstat("core");
  assert(coreInfo.isDirectory);
  assert(!coreInfo.isSymlink);

  const coreInfoByUrl = await Deno.lstat(pathToAbsoluteFileUrl("core"));
  assert(coreInfoByUrl.isDirectory);
  assert(!coreInfoByUrl.isSymlink);
});

unitTest({ perms: { read: false } }, async function lstatPerm(): Promise<void> {
  await assertThrowsAsync(async () => {
    await Deno.lstat("README.md");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, async function lstatNotFound(): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    await Deno.lstat("bad_file_name");
  }, Deno.errors.NotFound);
});

unitTest(
  { ignore: Deno.build.os !== "windows", perms: { read: true, write: true } },
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
  },
);

unitTest(
  { ignore: Deno.build.os === "windows", perms: { read: true, write: true } },
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
  },
);
