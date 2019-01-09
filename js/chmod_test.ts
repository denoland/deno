// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assertEqual } from "./test_util.ts";
import * as deno from "deno";

const isNotWindows = deno.platform.os !== "win";

testPerm({ write: true }, function chmodSyncSuccess() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const tempDir = deno.makeTempDirSync();
  const filename = tempDir + "/test.txt";
  deno.writeFileSync(filename, data, 0o666);

  // On windows no effect, but should not crash
  deno.chmodSync(filename, 0o777);

  // Check success when not on windows
  if (isNotWindows) {
    const fileInfo = deno.statSync(filename);
    assertEqual(fileInfo.mode & 0o777, 0o777);
  }
});

// Check symlink when not on windows
if (isNotWindows) {
  testPerm({ write: true }, function chmodSyncSymlinkSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = deno.makeTempDirSync();

    const filename = tempDir + "/test.txt";
    deno.writeFileSync(filename, data, 0o666);
    const symlinkName = tempDir + "/test_symlink.txt";
    deno.symlinkSync(filename, symlinkName);

    let symlinkInfo = deno.lstatSync(symlinkName);
    const symlinkMode = symlinkInfo.mode & 0o777; // platform dependent

    deno.chmodSync(symlinkName, 0o777);

    // Change actual file mode, not symlink
    const fileInfo = deno.statSync(filename);
    assertEqual(fileInfo.mode & 0o777, 0o777);
    symlinkInfo = deno.lstatSync(symlinkName);
    assertEqual(symlinkInfo.mode & 0o777, symlinkMode);
  });
}

testPerm({ write: true }, function chmodSyncFailure() {
  let err;
  try {
    const filename = "/badfile.txt";
    deno.chmodSync(filename, 0o777);
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: false }, function chmodSyncPerm() {
  let err;
  try {
    deno.chmodSync("/somefile.txt", 0o777);
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ write: true }, async function chmodSuccess() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const tempDir = deno.makeTempDirSync();
  const filename = tempDir + "/test.txt";
  deno.writeFileSync(filename, data, 0o666);

  // On windows no effect, but should not crash
  await deno.chmod(filename, 0o777);

  // Check success when not on windows
  if (isNotWindows) {
    const fileInfo = deno.statSync(filename);
    assertEqual(fileInfo.mode & 0o777, 0o777);
  }
});

// Check symlink when not on windows
if (isNotWindows) {
  testPerm({ write: true }, async function chmodSymlinkSuccess() {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const tempDir = deno.makeTempDirSync();

    const filename = tempDir + "/test.txt";
    deno.writeFileSync(filename, data, 0o666);
    const symlinkName = tempDir + "/test_symlink.txt";
    deno.symlinkSync(filename, symlinkName);

    let symlinkInfo = deno.lstatSync(symlinkName);
    const symlinkMode = symlinkInfo.mode & 0o777; // platform dependent

    await deno.chmod(symlinkName, 0o777);

    // Just change actual file mode, not symlink
    const fileInfo = deno.statSync(filename);
    assertEqual(fileInfo.mode & 0o777, 0o777);
    symlinkInfo = deno.lstatSync(symlinkName);
    assertEqual(symlinkInfo.mode & 0o777, symlinkMode);
  });
}

testPerm({ write: true }, async function chmodFailure() {
  let err;
  try {
    const filename = "/badfile.txt";
    await deno.chmod(filename, 0o777);
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: false }, async function chmodPerm() {
  let err;
  try {
    await deno.chmod("/somefile.txt", 0o777);
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});
