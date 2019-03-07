// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEquals } from "./test_util.ts";

// TODO Add tests for modified, accessed, and created fields once there is a way
// to create temp files.
testPerm({ read: true }, async function statSyncSuccess() {
  const packageInfo = Deno.statSync("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = Deno.statSync("testing");
  assert(testingInfo.isDirectory());
  assert(!testingInfo.isSymlink());

  const srcInfo = Deno.statSync("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

testPerm({ read: false }, async function statSyncPerm() {
  let caughtError = false;
  try {
    Deno.statSync("package.json");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true }, async function statSyncNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = Deno.statSync("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.NotFound);
    assertEquals(err.name, "NotFound");
  }

  assert(caughtError);
  assertEquals(badInfo, undefined);
});

testPerm({ read: true }, async function lstatSyncSuccess() {
  const packageInfo = Deno.lstatSync("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = Deno.lstatSync("testing");
  assert(!testingInfo.isDirectory());
  assert(testingInfo.isSymlink());

  const srcInfo = Deno.lstatSync("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

testPerm({ read: false }, async function lstatSyncPerm() {
  let caughtError = false;
  try {
    Deno.lstatSync("package.json");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true }, async function lstatSyncNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = Deno.lstatSync("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.NotFound);
    assertEquals(err.name, "NotFound");
  }

  assert(caughtError);
  assertEquals(badInfo, undefined);
});

testPerm({ read: true }, async function statSuccess() {
  const packageInfo = await Deno.stat("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = await Deno.stat("testing");
  assert(testingInfo.isDirectory());
  assert(!testingInfo.isSymlink());

  const srcInfo = await Deno.stat("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

testPerm({ read: false }, async function statPerm() {
  let caughtError = false;
  try {
    await Deno.stat("package.json");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true }, async function statNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = await Deno.stat("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.NotFound);
    assertEquals(err.name, "NotFound");
  }

  assert(caughtError);
  assertEquals(badInfo, undefined);
});

testPerm({ read: true }, async function lstatSuccess() {
  const packageInfo = await Deno.lstat("package.json");
  assert(packageInfo.isFile());
  assert(!packageInfo.isSymlink());

  const testingInfo = await Deno.lstat("testing");
  assert(!testingInfo.isDirectory());
  assert(testingInfo.isSymlink());

  const srcInfo = await Deno.lstat("src");
  assert(srcInfo.isDirectory());
  assert(!srcInfo.isSymlink());
});

testPerm({ read: false }, async function lstatPerm() {
  let caughtError = false;
  try {
    await Deno.lstat("package.json");
  } catch (e) {
    caughtError = true;
    assertEquals(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEquals(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true }, async function lstatNotFound() {
  let caughtError = false;
  let badInfo;

  try {
    badInfo = await Deno.lstat("bad_file_name");
  } catch (err) {
    caughtError = true;
    assertEquals(err.kind, Deno.ErrorKind.NotFound);
    assertEquals(err.name, "NotFound");
  }

  assert(caughtError);
  assertEquals(badInfo, undefined);
});
