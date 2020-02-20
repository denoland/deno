// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

const isNotWindows = Deno.build.os !== "win";

testPerm({ read: true, write: true }, function chmodSyncSuccess(): void {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const tempDir = Deno.makeTempDirSync();
  const filename = tempDir + "/test.txt";
  Deno.writeFileSync(filename, data, { perm: 0o666 });

  // On windows no effect, but should not crash
  Deno.chmodSync(filename, 0o777);

  // Check success when not on windows
  if (isNotWindows) {
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
  }
});

// Check symlink when not on windows
if (isNotWindows) {
  testPerm(
    { read: true, write: true },
    function chmodSyncSymlinkSuccess(): void {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const tempDir = Deno.makeTempDirSync();

      const filename = tempDir + "/test.txt";
      Deno.writeFileSync(filename, data, { perm: 0o666 });
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
}

testPerm({ write: true }, function chmodSyncFailure(): void {
  let err;
  try {
    const filename = "/badfile.txt";
    Deno.chmodSync(filename, 0o777);
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.NotFound);
  assertEquals(err.name, "NotFound");
});

testPerm({ write: false }, function chmodSyncPerm(): void {
  let err;
  try {
    Deno.chmodSync("/somefile.txt", 0o777);
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: true }, async function chmodSuccess(): Promise<
  void
> {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const tempDir = Deno.makeTempDirSync();
  const filename = tempDir + "/test.txt";
  Deno.writeFileSync(filename, data, { perm: 0o666 });

  // On windows no effect, but should not crash
  await Deno.chmod(filename, 0o777);

  // Check success when not on windows
  if (isNotWindows) {
    const fileInfo = Deno.statSync(filename);
    assert(fileInfo.mode);
    assertEquals(fileInfo.mode & 0o777, 0o777);
  }
});

// Check symlink when not on windows
if (isNotWindows) {
  testPerm(
    { read: true, write: true },
    async function chmodSymlinkSuccess(): Promise<void> {
      const enc = new TextEncoder();
      const data = enc.encode("Hello");
      const tempDir = Deno.makeTempDirSync();

      const filename = tempDir + "/test.txt";
      Deno.writeFileSync(filename, data, { perm: 0o666 });
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
}

testPerm({ write: true }, async function chmodFailure(): Promise<void> {
  let err;
  try {
    const filename = "/badfile.txt";
    await Deno.chmod(filename, 0o777);
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.NotFound);
  assertEquals(err.name, "NotFound");
});

testPerm({ write: false }, async function chmodPerm(): Promise<void> {
  let err;
  try {
    await Deno.chmod("/somefile.txt", 0o777);
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});
