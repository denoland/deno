// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assertEquals } from "./test_util.ts";

const isNotWindows = Deno.build.os !== "win";

testPerm({ read: true, write: true }, function chmodSyncSuccess() {
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
    assertEquals(fileInfo.mode & 0o777, 0o777);
  }
});

// Check symlink when not on windows
if (isNotWindows) {
  testPerm({ read: true, write: true }, function chmodSyncSymlinkSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();

    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { perm: 0o666 });
    const symlinkName = tempDir + "/test_symlink.txt";
    Deno.symlinkSync(filename, symlinkName);

    let symlinkInfo = Deno.lstatSync(symlinkName);
    const symlinkMode = symlinkInfo.mode & 0o777; // platform dependent

    Deno.chmodSync(symlinkName, 0o777);

    // Change actual file mode, not symlink
    const fileInfo = Deno.statSync(filename);
    assertEquals(fileInfo.mode & 0o777, 0o777);
    symlinkInfo = Deno.lstatSync(symlinkName);
    assertEquals(symlinkInfo.mode & 0o777, symlinkMode);
  });
}

testPerm({ write: true }, function chmodSyncFailure() {
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

testPerm({ write: false }, function chmodSyncPerm() {
  let err;
  try {
    Deno.chmodSync("/somefile.txt", 0o777);
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ read: true, write: true }, async function chmodSuccess() {
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
    assertEquals(fileInfo.mode & 0o777, 0o777);
  }
});

// Check symlink when not on windows
if (isNotWindows) {
  testPerm({ read: true, write: true }, async function chmodSymlinkSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = Deno.makeTempDirSync();

    const filename = tempDir + "/test.txt";
    Deno.writeFileSync(filename, data, { perm: 0o666 });
    const symlinkName = tempDir + "/test_symlink.txt";
    Deno.symlinkSync(filename, symlinkName);

    let symlinkInfo = Deno.lstatSync(symlinkName);
    const symlinkMode = symlinkInfo.mode & 0o777; // platform dependent

    await Deno.chmod(symlinkName, 0o777);

    // Just change actual file mode, not symlink
    const fileInfo = Deno.statSync(filename);
    assertEquals(fileInfo.mode & 0o777, 0o777);
    symlinkInfo = Deno.lstatSync(symlinkName);
    assertEquals(symlinkInfo.mode & 0o777, symlinkMode);
  });
}

testPerm({ write: true }, async function chmodFailure() {
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

testPerm({ write: false }, async function chmodPerm() {
  let err;
  try {
    await Deno.chmod("/somefile.txt", 0o777);
  } catch (e) {
    err = e;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});
