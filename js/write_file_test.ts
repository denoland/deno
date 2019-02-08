// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ read: true, write: true }, function writeFileSyncSuccess() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  deno.writeFileSync(filename, data);
  const dataRead = deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

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

testPerm({ write: false }, function writeFileSyncPerm() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  let caughtError = false;
  try {
    deno.writeFileSync(filename, data);
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true, write: true }, function writeFileSyncUpdatePerm() {
  if (deno.platform.os !== "win") {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = deno.makeTempDirSync() + "/test.txt";
    deno.writeFileSync(filename, data, { perm: 0o755 });
    assertEqual(deno.statSync(filename).mode & 0o777, 0o755);
    deno.writeFileSync(filename, data, { perm: 0o666 });
    assertEqual(deno.statSync(filename).mode & 0o777, 0o666);
  }
});

testPerm({ read: true, write: true }, function writeFileSyncCreate() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  let caughtError = false;
  // if create turned off, the file won't be created
  try {
    deno.writeFileSync(filename, data, { create: false });
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.NotFound);
    assertEqual(e.name, "NotFound");
  }
  assert(caughtError);

  // Turn on create, should have no error
  deno.writeFileSync(filename, data, { create: true });
  deno.writeFileSync(filename, data, { create: false });
  const dataRead = deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

testPerm({ read: true, write: true }, function writeFileSyncAppend() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  deno.writeFileSync(filename, data);
  deno.writeFileSync(filename, data, { append: true });
  let dataRead = deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  let actual = dec.decode(dataRead);
  assertEqual("HelloHello", actual);
  // Now attempt overwrite
  deno.writeFileSync(filename, data, { append: false });
  dataRead = deno.readFileSync(filename);
  actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
  // append not set should also overwrite
  deno.writeFileSync(filename, data);
  dataRead = deno.readFileSync(filename);
  actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

testPerm({ read: true, write: true }, async function writeFileSuccess() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  await deno.writeFile(filename, data);
  const dataRead = deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

testPerm({ read: true, write: true }, async function writeFileNotFound() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist (hopefully).
  let caughtError = false;
  try {
    await deno.writeFile(filename, data);
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.NotFound);
    assertEqual(e.name, "NotFound");
  }
  assert(caughtError);
});

testPerm({ read: true, write: false }, async function writeFilePerm() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  let caughtError = false;
  try {
    await deno.writeFile(filename, data);
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true, write: true }, async function writeFileUpdatePerm() {
  if (deno.platform.os !== "win") {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = deno.makeTempDirSync() + "/test.txt";
    await deno.writeFile(filename, data, { perm: 0o755 });
    assertEqual(deno.statSync(filename).mode & 0o777, 0o755);
    await deno.writeFile(filename, data, { perm: 0o666 });
    assertEqual(deno.statSync(filename).mode & 0o777, 0o666);
  }
});

testPerm({ read: true, write: true }, async function writeFileCreate() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  let caughtError = false;
  // if create turned off, the file won't be created
  try {
    await deno.writeFile(filename, data, { create: false });
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.NotFound);
    assertEqual(e.name, "NotFound");
  }
  assert(caughtError);

  // Turn on create, should have no error
  await deno.writeFile(filename, data, { create: true });
  await deno.writeFile(filename, data, { create: false });
  const dataRead = deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

testPerm({ read: true, write: true }, async function writeFileAppend() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = deno.makeTempDirSync() + "/test.txt";
  await deno.writeFile(filename, data);
  await deno.writeFile(filename, data, { append: true });
  let dataRead = deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  let actual = dec.decode(dataRead);
  assertEqual("HelloHello", actual);
  // Now attempt overwrite
  await deno.writeFile(filename, data, { append: false });
  dataRead = deno.readFileSync(filename);
  actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
  // append not set should also overwrite
  await deno.writeFile(filename, data);
  dataRead = deno.readFileSync(filename);
  actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});
