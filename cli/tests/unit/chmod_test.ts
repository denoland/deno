// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  unitTest,
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "./test_util.ts";

unitTest(
  { ignore: Deno.build.os === "windows", perms: { read: true, write: true } },
  function chmodSyncSuccess(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });

    Deno.chmodSync(filename, 0o777);

    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
  }
);

unitTest(
  { ignore: Deno.build.os === "windows", perms: { read: true, write: true } },
  function chmodSyncUrl(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(`file://${tempDir}/test.txt`);
    Deno.writeFileSync(fileUrl, data, { mode: 0o666 });

    Deno.chmodSync(fileUrl, 0o777);

    const fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);

    Deno.removeSync(tempDir, { recursive: true });
  }
);

// Check symlink when not on windows
unitTest(
  {
    ignore: Deno.build.os === "windows",
    perms: { read: true, write: true },
  },
  function chmodSyncSymlinkSuccess(): void {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();

    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const symlinkName = tempDir + "/test_symlink.txt";
    Deno.symlinkSync(filename, symlinkName);

    let symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    const symlinkMode = symlinkInfo.mode & 0o777; // platform dependent

    Deno.chmodSync(symlinkName, 0o777);

    // Change actual file mode, not symlink
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
    symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    assertEquals(symlinkInfo.mode & 0o777, symlinkMode);
  }
);

unitTest({ perms: { write: true } }, function chmodSyncFailure(): void {
  assertThrows(() => {
    const filename = "/badfile.txt";
    Deno.chmodSync(filename, 0o777);
  }, Deno.errors.NotFound);
});

unitTest({ perms: { write: false } }, function chmodSyncPerm(): void {
  assertThrows(() => {
    Deno.chmodSync("/somefile.txt", 0o777);
  }, Deno.errors.PermissionDenied);
});

unitTest(
  { ignore: Deno.build.os === "windows", perms: { read: true, write: true } },
  async function chmodSuccess(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });

    await Deno.chmod(filename, 0o777);

    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
  }
);

unitTest(
  { ignore: Deno.build.os === "windows", perms: { read: true, write: true } },
  async function chmodUrl(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(`file://${tempDir}/test.txt`);
    Deno.writeFileSync(fileUrl, data, { mode: 0o666 });

    await Deno.chmod(fileUrl, 0o777);

    const fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);

    Deno.removeSync(tempDir, { recursive: true });
  }
);

// Check symlink when not on windows

unitTest(
  {
    ignore: Deno.build.os === "windows",
    perms: { read: true, write: true },
  },
  async function chmodSymlinkSuccess(): Promise<void> {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();

    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { mode: 0o666 });
    const symlinkName = tempDir + "/test_symlink.txt";
    Deno.symlinkSync(filename, symlinkName);

    let symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    const symlinkMode = symlinkInfo.mode & 0o777; // platform dependent

    await Deno.chmod(symlinkName, 0o777);

    // Just change actual file mode, not symlink
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
    symlinkInfo = Deno.lstatSync(symlinkName);
    assert(symlinkInfo.mode);
    assertEquals(symlinkInfo.mode & 0o777, symlinkMode);
  }
);

unitTest({ perms: { write: true } }, async function chmodFailure(): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    const filename = "/badfile.txt";
    await Deno.chmod(filename, 0o777);
  }, Deno.errors.NotFound);
});

unitTest({ perms: { write: false } }, async function chmodPerm(): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    await Deno.chmod("/somefile.txt", 0o777);
  }, Deno.errors.PermissionDenied);
});
