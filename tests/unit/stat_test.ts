// Copyright 2018-2026 the Deno authors. MIT license.

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
    const now = Date.now();
    assert(tempInfo.atime !== null && now - tempInfo.atime.valueOf() < 60_000);
    assert(tempInfo.mtime !== null && now - tempInfo.mtime.valueOf() < 60_000);
    assert(tempInfo.ctime !== null && now - tempInfo.ctime.valueOf() < 60_000);
    const mode = tempInfo.mode! & 0o777;
    if (Deno.build.os === "windows") {
      assertEquals(mode, 0o666);
    } else {
      assertEquals(mode, 0o600);
    }

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
    assert(tempInfoByUrl.atime !== null);
    assert(tempInfoByUrl.mtime !== null);
    assert(tempInfoByUrl.ctime !== null);

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
    assert(tempInfo.atime !== null);
    assert(tempInfo.mtime !== null);
    assert(tempInfo.ctime !== null);

    const tempFileForUrl = await Deno.makeTempFile();
    const tempInfoByUrl = await Deno.stat(
      new URL(
        `file://${Deno.build.os === "windows" ? "/" : ""}${tempFileForUrl}`,
      ),
    );
    assert(tempInfoByUrl.atime !== null);
    assert(tempInfoByUrl.mtime !== null);
    assert(tempInfoByUrl.ctime !== null);
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
    assert(s.ino !== null);
    assert(s.nlink !== null);
    assert(s.uid === null);
    assert(s.gid === null);
    assert(s.rdev === null);
    assert(s.blksize === null);
    assert(s.blocks !== null);
    assert(s.isBlockDevice === null);
    assert(s.isCharDevice === null);
    assert(s.isFifo === null);
    assert(s.isSocket === null);
  },
);

Deno.test(
  {
    ignore: Deno.build.os !== "windows",
    permissions: { ffi: true, read: true, write: true },
  },
  function statAppExecLink() {
    const tempDir = Deno.realPathSync(Deno.makeTempDirSync());
    const path = `${tempDir}\\app_exec_alias.exe`;
    Deno.writeFileSync(path, new Uint8Array());
    createAppExecLink(path, Deno.execPath());

    // an app execution alias is a reparse point that only `CreateProcess`
    // knows how to follow, so the file system reports it as an empty file and
    // `stat` does not differ from `lstat`
    for (const info of [Deno.statSync(path), Deno.lstatSync(path)]) {
      assert(info.isFile);
      assert(!info.isDirectory);
      assert(!info.isSymlink);
      assertEquals(info.size, 0);
    }
    assertEquals(Deno.realPathSync(path), path);
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

Deno.test(
  { permissions: { read: false, write: true } },
  async function fsFileStatFailPermissions() {
    const testDir = Deno.makeTempDirSync();
    const filename = testDir + "/file.txt";
    using file = await Deno.open(filename, {
      read: false,
      write: true,
      create: true,
    });

    await assertRejects(
      () => file.stat(),
      Deno.errors.NotCapable,
      "Requires read access to",
    );
    assertThrows(
      () => file.statSync(),
      Deno.errors.NotCapable,
      "Requires read access to",
    );
  },
);

/**
 * Turns the empty file at `path` into a Windows app execution alias pointing
 * at `target`, the kind of reparse point Microsoft Store apps install into
 * `%LOCALAPPDATA%\Microsoft\WindowsApps`.
 */
function createAppExecLink(path: string, target: string) {
  const GENERIC_WRITE = 0x40000000;
  const FILE_SHARE_ALL = 0x00000007;
  const OPEN_EXISTING = 3;
  const FILE_FLAG_BACKUP_SEMANTICS = 0x02000000;
  const FILE_FLAG_OPEN_REPARSE_POINT = 0x00200000;
  const FSCTL_SET_REPARSE_POINT = 0x000900a4;
  const IO_REPARSE_TAG_APPEXECLINK = 0x8000001b;

  // the tag's data is a version followed by a NUL separated list of strings:
  // the package family name, the app user model id, the target and the
  // application type
  const family = "Deno.Test_8wekyb3d8bbwe";
  const strings = wideCString(`${family}\0${family}!App\0${target}\0` + "0");
  const reparseData = new Uint8Array(8 + 4 + strings.byteLength);
  const view = new DataView(reparseData.buffer);
  view.setUint32(0, IO_REPARSE_TAG_APPEXECLINK, true);
  view.setUint16(4, 4 + strings.byteLength, true); // ReparseDataLength
  view.setUint32(8, 3, true); // Version
  reparseData.set(strings, 12);

  const kernel32 = Deno.dlopen("kernel32.dll", {
    CreateFileW: {
      parameters: ["buffer", "u32", "u32", "pointer", "u32", "u32", "pointer"],
      result: "pointer",
    },
    DeviceIoControl: {
      parameters: [
        "pointer",
        "u32",
        "buffer",
        "u32",
        "pointer",
        "u32",
        "buffer",
        "pointer",
      ],
      result: "i32",
    },
    CloseHandle: { parameters: ["pointer"], result: "i32" },
  });
  try {
    const file = kernel32.symbols.CreateFileW(
      wideCString(path),
      GENERIC_WRITE,
      FILE_SHARE_ALL,
      null,
      OPEN_EXISTING,
      FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
      null,
    );
    try {
      const result = kernel32.symbols.DeviceIoControl(
        file,
        FSCTL_SET_REPARSE_POINT,
        reparseData,
        reparseData.byteLength,
        null,
        0,
        new Uint8Array(4),
        null,
      );
      // fails if `CreateFileW` above failed as well, because then `file` is
      // `INVALID_HANDLE_VALUE`
      assert(result !== 0, "failed to create the app execution alias");
    } finally {
      kernel32.symbols.CloseHandle(file);
    }
  } finally {
    kernel32.close();
  }
}

function wideCString(value: string) {
  const wide = new Uint16Array(value.length + 1);
  for (let i = 0; i < value.length; i++) {
    wide[i] = value.charCodeAt(i);
  }
  return new Uint8Array(wide.buffer);
}
