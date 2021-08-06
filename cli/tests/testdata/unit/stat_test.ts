// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

unitTest({ perms: { read: true } }, function fstatSyncSuccess() {
  const file = Deno.openSync("hello.txt");
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

unitTest({ perms: { read: true } }, async function fstatSuccess() {
  const file = await Deno.open("hello.txt");
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
  function statSyncSuccess() {
    const helloInfo = Deno.statSync("hello.txt");
    assert(helloInfo.isFile);
    assert(!helloInfo.isSymlink);

    const modulesInfo = Deno.statSync("symlink_to_subdir");
    assert(modulesInfo.isDirectory);
    assert(!modulesInfo.isSymlink);

    const subdirInfo = Deno.statSync("subdir");
    assert(subdirInfo.isDirectory);
    assert(!subdirInfo.isSymlink);

    const tempFile = Deno.makeTempFileSync();
    const tempInfo = Deno.statSync(tempFile);
    let now = Date.now();
    assert(tempInfo.atime !== null && now - tempInfo.atime.valueOf() < 1000);
    assert(tempInfo.mtime !== null && now - tempInfo.mtime.valueOf() < 1000);
    assert(
      tempInfo.birthtime === null || now - tempInfo.birthtime.valueOf() < 1000,
    );

    const packageInfoByUrl = Deno.statSync(pathToAbsoluteFileUrl("hello.txt"));
    assert(packageInfoByUrl.isFile);
    assert(!packageInfoByUrl.isSymlink);

    const modulesInfoByUrl = Deno.statSync(
      pathToAbsoluteFileUrl("symlink_to_subdir"),
    );
    assert(modulesInfoByUrl.isDirectory);
    assert(!modulesInfoByUrl.isSymlink);

    const subDirInfoByUrl = Deno.statSync(pathToAbsoluteFileUrl("subdir"));
    assert(subDirInfoByUrl.isDirectory);
    assert(!subDirInfoByUrl.isSymlink);

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

unitTest({ perms: { read: false } }, function statSyncPerm() {
  assertThrows(() => {
    Deno.statSync("hello.txt");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function statSyncNotFound() {
  assertThrows(() => {
    Deno.statSync("bad_file_name");
  }, Deno.errors.NotFound);
});

unitTest({ perms: { read: true } }, function lstatSyncSuccess() {
  const packageInfo = Deno.lstatSync("hello.txt");
  assert(packageInfo.isFile);
  assert(!packageInfo.isSymlink);

  const packageInfoByUrl = Deno.lstatSync(pathToAbsoluteFileUrl("hello.txt"));
  assert(packageInfoByUrl.isFile);
  assert(!packageInfoByUrl.isSymlink);

  const modulesInfo = Deno.lstatSync("symlink_to_subdir");
  assert(!modulesInfo.isDirectory);
  assert(modulesInfo.isSymlink);

  const modulesInfoByUrl = Deno.lstatSync(
    pathToAbsoluteFileUrl("symlink_to_subdir"),
  );
  assert(!modulesInfoByUrl.isDirectory);
  assert(modulesInfoByUrl.isSymlink);

  const subDirInfo = Deno.lstatSync("subdir");
  assert(subDirInfo.isDirectory);
  assert(!subDirInfo.isSymlink);

  const subDirInfoByUrl = Deno.lstatSync(pathToAbsoluteFileUrl("subdir"));
  assert(subDirInfoByUrl.isDirectory);
  assert(!subDirInfoByUrl.isSymlink);
});

unitTest({ perms: { read: false } }, function lstatSyncPerm() {
  assertThrows(() => {
    Deno.lstatSync("hello.txt");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function lstatSyncNotFound() {
  assertThrows(() => {
    Deno.lstatSync("bad_file_name");
  }, Deno.errors.NotFound);
});

unitTest(
  { perms: { read: true, write: true } },
  async function statSuccess() {
    const packageInfo = await Deno.stat("hello.txt");
    assert(packageInfo.isFile);
    assert(!packageInfo.isSymlink);

    const packageInfoByUrl = await Deno.stat(
      pathToAbsoluteFileUrl("hello.txt"),
    );
    assert(packageInfoByUrl.isFile);
    assert(!packageInfoByUrl.isSymlink);

    const modulesInfo = await Deno.stat("symlink_to_subdir");
    assert(modulesInfo.isDirectory);
    assert(!modulesInfo.isSymlink);

    const modulesInfoByUrl = await Deno.stat(
      pathToAbsoluteFileUrl("symlink_to_subdir"),
    );
    assert(modulesInfoByUrl.isDirectory);
    assert(!modulesInfoByUrl.isSymlink);

    const subDirInfo = await Deno.stat("subdir");
    assert(subDirInfo.isDirectory);
    assert(!subDirInfo.isSymlink);

    const subDirInfoByUrl = await Deno.stat(pathToAbsoluteFileUrl("subdir"));
    assert(subDirInfoByUrl.isDirectory);
    assert(!subDirInfoByUrl.isSymlink);

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

unitTest({ perms: { read: false } }, async function statPerm() {
  await assertThrowsAsync(async () => {
    await Deno.stat("hello.txt");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, async function statNotFound() {
  await assertThrowsAsync(
    async () => {
      await Deno.stat("bad_file_name"), Deno.errors.NotFound;
    },
  );
});

unitTest({ perms: { read: true } }, async function lstatSuccess() {
  const packageInfo = await Deno.lstat("hello.txt");
  assert(packageInfo.isFile);
  assert(!packageInfo.isSymlink);

  const packageInfoByUrl = await Deno.lstat(pathToAbsoluteFileUrl("hello.txt"));
  assert(packageInfoByUrl.isFile);
  assert(!packageInfoByUrl.isSymlink);

  const modulesInfo = await Deno.lstat("symlink_to_subdir");
  assert(!modulesInfo.isDirectory);
  assert(modulesInfo.isSymlink);

  const modulesInfoByUrl = await Deno.lstat(
    pathToAbsoluteFileUrl("symlink_to_subdir"),
  );
  assert(!modulesInfoByUrl.isDirectory);
  assert(modulesInfoByUrl.isSymlink);

  const subDirInfo = await Deno.lstat("subdir");
  assert(subDirInfo.isDirectory);
  assert(!subDirInfo.isSymlink);

  const subDirInfoByUrl = await Deno.lstat(pathToAbsoluteFileUrl("subdir"));
  assert(subDirInfoByUrl.isDirectory);
  assert(!subDirInfoByUrl.isSymlink);
});

unitTest({ perms: { read: false } }, async function lstatPerm() {
  await assertThrowsAsync(async () => {
    await Deno.lstat("hello.txt");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, async function lstatNotFound() {
  await assertThrowsAsync(async () => {
    await Deno.lstat("bad_file_name");
  }, Deno.errors.NotFound);
});

unitTest(
  { ignore: Deno.build.os !== "windows", perms: { read: true, write: true } },
  function statNoUnixFields() {
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
  function statUnixFields() {
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
