// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

// SYNC

testPerm({ write: true }, function removeSyncDirSuccess() {
  // REMOVE EMPTY DIRECTORY
  const path = deno.makeTempDirSync() + "/dir/subdir";
  deno.mkdirSync(path);
  const pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory()); // check exist first
  deno.removeSync(path); // remove
  // We then check again after remove
  let err;
  try {
    deno.statSync(path);
  } catch (e) {
    err = e;
  }
  // Directory is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: true }, function removeSyncFileSuccess() {
  // REMOVE FILE
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  deno.writeFileSync(filename, data, 0o666);
  const fileInfo = deno.statSync(filename);
  assert(fileInfo.isFile()); // check exist first
  deno.removeSync(filename); // remove
  // We then check again after remove
  let err;
  try {
    deno.statSync(filename);
  } catch (e) {
    err = e;
  }
  // File is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: true }, function removeSyncFail() {
  // NON-EMPTY DIRECTORY
  const path = deno.makeTempDirSync() + "/dir/subdir";
  const subPath = path + "/subsubdir";
  deno.mkdirSync(path);
  deno.mkdirSync(subPath);
  const pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory()); // check exist first
  const subPathInfo = deno.statSync(subPath);
  assert(subPathInfo.isDirectory()); // check exist first
  let err;
  try {
    // Should not be able to recursively remove
    deno.removeSync(path);
  } catch (e) {
    err = e;
  }
  // TODO(ry) Is Other really the error we should get here? What would Go do?
  assertEqual(err.kind, deno.ErrorKind.Other);
  assertEqual(err.name, "Other");
  // NON-EXISTENT DIRECTORY/FILE
  try {
    // Non-existent
    deno.removeSync("/baddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: false }, function removeSyncPerm() {
  let err;
  try {
    deno.removeSync("/baddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ write: true }, function removeAllSyncDirSuccess() {
  // REMOVE EMPTY DIRECTORY
  let path = deno.makeTempDirSync() + "/dir/subdir";
  deno.mkdirSync(path);
  let pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory()); // check exist first
  deno.removeAllSync(path); // remove
  // We then check again after remove
  let err;
  try {
    deno.statSync(path);
  } catch (e) {
    err = e;
  }
  // Directory is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
  // REMOVE NON-EMPTY DIRECTORY
  path = deno.makeTempDirSync() + "/dir/subdir";
  const subPath = path + "/subsubdir";
  deno.mkdirSync(path);
  deno.mkdirSync(subPath);
  pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory()); // check exist first
  const subPathInfo = deno.statSync(subPath);
  assert(subPathInfo.isDirectory()); // check exist first
  deno.removeAllSync(path); // remove
  // We then check parent directory again after remove
  try {
    deno.statSync(path);
  } catch (e) {
    err = e;
  }
  // Directory is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: true }, function removeAllSyncFileSuccess() {
  // REMOVE FILE
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  deno.writeFileSync(filename, data, 0o666);
  const fileInfo = deno.statSync(filename);
  assert(fileInfo.isFile()); // check exist first
  deno.removeAllSync(filename); // remove
  // We then check again after remove
  let err;
  try {
    deno.statSync(filename);
  } catch (e) {
    err = e;
  }
  // File is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: true }, function removeAllSyncFail() {
  // NON-EXISTENT DIRECTORY/FILE
  let err;
  try {
    // Non-existent
    deno.removeAllSync("/baddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: false }, function removeAllSyncPerm() {
  let err;
  try {
    deno.removeAllSync("/baddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

// ASYNC

testPerm({ write: true }, async function removeDirSuccess() {
  // REMOVE EMPTY DIRECTORY
  const path = deno.makeTempDirSync() + "/dir/subdir";
  deno.mkdirSync(path);
  const pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory()); // check exist first
  await deno.remove(path); // remove
  // We then check again after remove
  let err;
  try {
    deno.statSync(path);
  } catch (e) {
    err = e;
  }
  // Directory is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: true }, async function removeFileSuccess() {
  // REMOVE FILE
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  deno.writeFileSync(filename, data, 0o666);
  const fileInfo = deno.statSync(filename);
  assert(fileInfo.isFile()); // check exist first
  await deno.remove(filename); // remove
  // We then check again after remove
  let err;
  try {
    deno.statSync(filename);
  } catch (e) {
    err = e;
  }
  // File is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: true }, async function removeFail() {
  // NON-EMPTY DIRECTORY
  const path = deno.makeTempDirSync() + "/dir/subdir";
  const subPath = path + "/subsubdir";
  deno.mkdirSync(path);
  deno.mkdirSync(subPath);
  const pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory()); // check exist first
  const subPathInfo = deno.statSync(subPath);
  assert(subPathInfo.isDirectory()); // check exist first
  let err;
  try {
    // Should not be able to recursively remove
    await deno.remove(path);
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.Other);
  assertEqual(err.name, "Other");
  // NON-EXISTENT DIRECTORY/FILE
  try {
    // Non-existent
    await deno.remove("/baddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: false }, async function removePerm() {
  let err;
  try {
    await deno.remove("/baddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ write: true }, async function removeAllDirSuccess() {
  // REMOVE EMPTY DIRECTORY
  let path = deno.makeTempDirSync() + "/dir/subdir";
  deno.mkdirSync(path);
  let pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory()); // check exist first
  await deno.removeAll(path); // remove
  // We then check again after remove
  let err;
  try {
    deno.statSync(path);
  } catch (e) {
    err = e;
  }
  // Directory is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
  // REMOVE NON-EMPTY DIRECTORY
  path = deno.makeTempDirSync() + "/dir/subdir";
  const subPath = path + "/subsubdir";
  deno.mkdirSync(path);
  deno.mkdirSync(subPath);
  pathInfo = deno.statSync(path);
  assert(pathInfo.isDirectory()); // check exist first
  const subPathInfo = deno.statSync(subPath);
  assert(subPathInfo.isDirectory()); // check exist first
  await deno.removeAll(path); // remove
  // We then check parent directory again after remove
  try {
    deno.statSync(path);
  } catch (e) {
    err = e;
  }
  // Directory is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: true }, async function removeAllFileSuccess() {
  // REMOVE FILE
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  deno.writeFileSync(filename, data, 0o666);
  const fileInfo = deno.statSync(filename);
  assert(fileInfo.isFile()); // check exist first
  await deno.removeAll(filename); // remove
  // We then check again after remove
  let err;
  try {
    deno.statSync(filename);
  } catch (e) {
    err = e;
  }
  // File is gone
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: true }, async function removeAllFail() {
  // NON-EXISTENT DIRECTORY/FILE
  let err;
  try {
    // Non-existent
    await deno.removeAll("/baddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});

testPerm({ write: false }, async function removeAllPerm() {
  let err;
  try {
    await deno.removeAll("/baddir");
  } catch (e) {
    err = e;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});
