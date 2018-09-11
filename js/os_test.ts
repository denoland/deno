// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ env: true }, async function envSuccess() {
  const env = deno.env();
  assert(env !== null);
  env.test_var = "Hello World";
  const newEnv = deno.env();
  assertEqual(env.test_var, newEnv.test_var);
});

test(async function envFailure() {
  let caughtError = false;
  try {
    const env = deno.env();
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
    assertEqual(err.name, "PermissionDenied");
  }

  assert(caughtError);
});

// TODO Add tests for modified, accessed, and created fields once there is a way
// to create temp files.
test(async function statSyncSuccess() {
  const packageInfo = deno.statSync("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = deno.statSync("testing");
  assert(testingInfo.isDirectory());
  assert(!testingInfo.isSymlink());

  const srcInfo = deno.statSync("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

test(async function statSyncNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = deno.statSync("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.NotFound);
    assertEqual(err.name, "NotFound");
  }

  assert(caughtError);
  assertEqual(badInfo, undefined);
});

test(async function lstatSyncSuccess() {
  const packageInfo = deno.lstatSync("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = deno.lstatSync("testing");
  assert(!testingInfo.isDirectory());
  assert(testingInfo.isSymlink());

  const srcInfo = deno.lstatSync("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

test(async function lstatSyncNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = deno.lstatSync("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEqual(err.kind, deno.ErrorKind.NotFound);
    assertEqual(err.name, "NotFound");
  }

  assert(caughtError);
  assertEqual(badInfo, undefined);
});

testPerm({ write: true }, function writeFileSyncSuccess() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  deno.writeFileSync(filename, data, 0o666);
  const dataRead = deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

// For this test to pass we need --allow-write permission.
// Otherwise it will fail with deno.PermissionDenied instead of deno.NotFound.
testPerm({ write: true }, function writeFileSyncFail() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist (hopefully).
  let caughtError = false;
  try {
    deno.writeFileSync(filename, data);
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.NotFound);
    assertEqual(e.name, "NotFound");
  }
  assert(caughtError);
});

testPerm({ write: true }, function makeTempDirSync() {
  const dir1 = deno.makeTempDirSync({ prefix: "hello", suffix: "world" });
  const dir2 = deno.makeTempDirSync({ prefix: "hello", suffix: "world" });
  // Check that both dirs are different.
  assert(dir1 != dir2);
  for (const dir of [dir1, dir2]) {
    // Check that the prefix and suffix are applied.
    const lastPart = dir.replace(/^.*[\\\/]/, "");
    assert(lastPart.startsWith("hello"));
    assert(lastPart.endsWith("world"));
  }
  // Check that the `dir` option works.
  const dir3 = deno.makeTempDirSync({ dir: dir1 });
  assert(dir3.startsWith(dir1));
  assert(/^[\\\/]/.test(dir3.slice(dir1.length)));
  // Check that creating a temp dir inside a nonexisting directory fails.
  let err;
  try {
    deno.makeTempDirSync({ dir: "/baddir" });
  } catch (err_) {
    err = err_;
  }
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

test(function makeTempDirSyncPerm() {
  // makeTempDirSync should require write permissions (for now).
  let err;
  try {
    deno.makeTempDirSync({ dir: "/baddir" });
  } catch (err_) {
    err = err_;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ write: true }, function renameSync() {
  const testDir = deno.makeTempDirSync() + "/test-rename";
  const oldpath = testDir + "/oldpath";
  const newpath = testDir + "/newpath";
  deno.mkdirSync(oldpath);
  deno.renameSync(oldpath, newpath);
  const newPathInfo = deno.statSync(newpath);
  assert(newPathInfo.isDirectory());

  let caughtErr = false;
  let oldPathInfo;

  try {
    oldPathInfo = deno.statSync(oldpath);
  } catch (err) {
    caughtErr = true;
    assertEqual(err.kind, deno.ErrorKind.NotFound);
    assertEqual(err.name, "NotFound");
  }

  assert(caughtErr);
  assertEqual(oldPathInfo, undefined);
});

test(function renameSyncPerm() {
  let err;
  try {
    const oldpath = "/oldbaddir";
    const newpath = "/newbaddir";
    deno.renameSync(oldpath, newpath);
  } catch (err_) {
    err = err_;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});
