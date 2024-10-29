// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

Deno.test(
  { permissions: { read: true, write: true } },
  function statSyncSuccess() {
    const readmeInfo = Deno.statSync("README.md");
    assert(readmeInfo.isFile);
    assert(!readmeInfo.isSymlink);

    const modulesInfo = Deno.statSync("tests/testdata/symlink_to_subdir");
    assert(modulesInfo.isDirectory);
    assert(!modulesInfo.isSymlink);

    const testsInfo = Deno.statSync("tests");
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

    const readmeInfoByUrl = Deno.statSync(pathToAbsoluteFileUrl("README.md"));
    assert(readmeInfoByUrl.isFile);
    assert(!readmeInfoByUrl.isSymlink);

    const modulesInfoByUrl = Deno.statSync(
      pathToAbsoluteFileUrl("tests/testdata/symlink_to_subdir"),
    );
    assert(modulesInfoByUrl.isDirectory);
    assert(!modulesInfoByUrl.isSymlink);

    const testsInfoByUrl = Deno.statSync(pathToAbsoluteFileUrl("tests"));
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

Deno.test({ permissions: { read: false } }, function statSyncPerm() {
  assertThrows(() => {
    Deno.statSync("README.md");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, function statSyncNotFound() {
  assertThrows(
    () => {
      Deno.statSync("bad_file_name");
    },
    Deno.errors.NotFound,
    `stat 'bad_file_name'`,
  );
});

Deno.test({ permissions: { read: true } }, function lstatSyncSuccess() {
  const packageInfo = Deno.lstatSync("README.md");
  assert(packageInfo.isFile);
  assert(!packageInfo.isSymlink);

  const packageInfoByUrl = Deno.lstatSync(pathToAbsoluteFileUrl("README.md"));
  assert(packageInfoByUrl.isFile);
  assert(!packageInfoByUrl.isSymlink);

  const modulesInfo = Deno.lstatSync("tests/testdata/symlink_to_subdir");
  assert(!modulesInfo.isDirectory);
  assert(modulesInfo.isSymlink);

  const modulesInfoByUrl = Deno.lstatSync(
    pathToAbsoluteFileUrl("tests/testdata/symlink_to_subdir"),
  );
  assert(!modulesInfoByUrl.isDirectory);
  assert(modulesInfoByUrl.isSymlink);

  const coreInfo = Deno.lstatSync("cli");
  assert(coreInfo.isDirectory);
  assert(!coreInfo.isSymlink);

  const coreInfoByUrl = Deno.lstatSync(pathToAbsoluteFileUrl("cli"));
  assert(coreInfoByUrl.isDirectory);
  assert(!coreInfoByUrl.isSymlink);
});

Deno.test({ permissions: { read: false } }, function lstatSyncPerm() {
  assertThrows(() => {
    Deno.lstatSync("assets/hello.txt");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, function lstatSyncNotFound() {
  assertThrows(
    () => {
      Deno.lstatSync("bad_file_name");
    },
    Deno.errors.NotFound,
    `stat 'bad_file_name'`,
  );
});

Deno.test(
  { permissions: { read: true, write: true } },
  async function statSuccess() {
    const readmeInfo = await Deno.stat("README.md");
    assert(readmeInfo.isFile);
    assert(!readmeInfo.isSymlink);

    const readmeInfoByUrl = await Deno.stat(
      pathToAbsoluteFileUrl("README.md"),
    );
    assert(readmeInfoByUrl.isFile);
    assert(!readmeInfoByUrl.isSymlink);

    const modulesInfo = await Deno.stat("tests/testdata/symlink_to_subdir");
    assert(modulesInfo.isDirectory);
    assert(!modulesInfo.isSymlink);

    const modulesInfoByUrl = await Deno.stat(
      pathToAbsoluteFileUrl("tests/testdata/symlink_to_subdir"),
    );
    assert(modulesInfoByUrl.isDirectory);
    assert(!modulesInfoByUrl.isSymlink);

    const testsInfo = await Deno.stat("tests");
    assert(testsInfo.isDirectory);
    assert(!testsInfo.isSymlink);

    const testsInfoByUrl = await Deno.stat(pathToAbsoluteFileUrl("tests"));
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

Deno.test({ permissions: { read: false } }, async function statPerm() {
  await assertRejects(async () => {
    await Deno.stat("README.md");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, async function statNotFound() {
  await assertRejects(
    async () => {
      await Deno.stat("bad_file_name");
    },
    Deno.errors.NotFound,
    `stat 'bad_file_name'`,
  );
});

Deno.test({ permissions: { read: true } }, async function lstatSuccess() {
  const readmeInfo = await Deno.lstat("README.md");
  assert(readmeInfo.isFile);
  assert(!readmeInfo.isSymlink);

  const readmeInfoByUrl = await Deno.lstat(pathToAbsoluteFileUrl("README.md"));
  assert(readmeInfoByUrl.isFile);
  assert(!readmeInfoByUrl.isSymlink);

  const modulesInfo = await Deno.lstat("tests/testdata/symlink_to_subdir");
  assert(!modulesInfo.isDirectory);
  assert(modulesInfo.isSymlink);

  const modulesInfoByUrl = await Deno.lstat(
    pathToAbsoluteFileUrl("tests/testdata/symlink_to_subdir"),
  );
  assert(!modulesInfoByUrl.isDirectory);
  assert(modulesInfoByUrl.isSymlink);

  const coreInfo = await Deno.lstat("cli");
  assert(coreInfo.isDirectory);
  assert(!coreInfo.isSymlink);

  const coreInfoByUrl = await Deno.lstat(pathToAbsoluteFileUrl("cli"));
  assert(coreInfoByUrl.isDirectory);
  assert(!coreInfoByUrl.isSymlink);
});

Deno.test({ permissions: { read: false } }, async function lstatPerm() {
  await assertRejects(async () => {
    await Deno.lstat("README.md");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, async function lstatNotFound() {
  await assertRejects(
    async () => {
      await Deno.lstat("bad_file_name");
    },
    Deno.errors.NotFound,
    `stat 'bad_file_name'`,
  );
});

Deno.test(
  {
    ignore: Deno.build.os !== "windows",
    permissions: { read: true, write: true },
  },
  function statNoUnixFields() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const s = Deno.statSync(filename);
    assert(s.dev !== 0);
    assert(s.ino === null);
    assert(s.mode === null);
    assert(s.nlink === null);
    assert(s.uid === null);
    assert(s.gid === null);
    assert(s.rdev === null);
    assert(s.blksize === null);
    assert(s.blocks === null);
    assert(s.isBlockDevice === null);
    assert(s.isCharDevice === null);
    assert(s.isFifo === null);
    assert(s.isSocket === null);
  },
);

Deno.test(
  {
    ignore: Deno.build.os === "windows",
    permissions: { read: true, write: true },
  },
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
    assert(!s.isBlockDevice);
    assert(!s.isCharDevice);
    assert(!s.isFifo);
    assert(!s.isSocket);
  },
);
