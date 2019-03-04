// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert, assertEqual } from "./test_util.ts";

testPerm({ read: true, write: true }, function writeFileSyncSuccess() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test.txt";
  Deno.writeFileSync(filename, data);
  const dataRead = Deno.readFileSync(filename);
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
    Deno.writeFileSync(filename, data);
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.NotFound);
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
    Deno.writeFileSync(filename, data);
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true, write: true }, function writeFileSyncUpdatePerm() {
  if (Deno.build.os !== "win") {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeFileSync(filename, data, { perm: 0o755 });
    assertEqual(Deno.statSync(filename).mode & 0o777, 0o755);
    Deno.writeFileSync(filename, data, { perm: 0o666 });
    assertEqual(Deno.statSync(filename).mode & 0o777, 0o666);
  }
});

testPerm({ read: true, write: true }, function writeFileSyncCreate() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test.txt";
  let caughtError = false;
  // if create turned off, the file won't be created
  try {
    Deno.writeFileSync(filename, data, { create: false });
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.NotFound);
    assertEqual(e.name, "NotFound");
  }
  assert(caughtError);

  // Turn on create, should have no error
  Deno.writeFileSync(filename, data, { create: true });
  Deno.writeFileSync(filename, data, { create: false });
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

testPerm({ read: true, write: true }, function writeFileSyncAppend() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test.txt";
  Deno.writeFileSync(filename, data);
  Deno.writeFileSync(filename, data, { append: true });
  let dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  let actual = dec.decode(dataRead);
  assertEqual("HelloHello", actual);
  // Now attempt overwrite
  Deno.writeFileSync(filename, data, { append: false });
  dataRead = Deno.readFileSync(filename);
  actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
  // append not set should also overwrite
  Deno.writeFileSync(filename, data);
  dataRead = Deno.readFileSync(filename);
  actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

testPerm({ read: true, write: true }, async function writeFileSuccess() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test.txt";
  await Deno.writeFile(filename, data);
  const dataRead = Deno.readFileSync(filename);
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
    await Deno.writeFile(filename, data);
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.NotFound);
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
    await Deno.writeFile(filename, data);
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});

testPerm({ read: true, write: true }, async function writeFileUpdatePerm() {
  if (Deno.build.os !== "win") {
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeFile(filename, data, { perm: 0o755 });
    assertEqual(Deno.statSync(filename).mode & 0o777, 0o755);
    await Deno.writeFile(filename, data, { perm: 0o666 });
    assertEqual(Deno.statSync(filename).mode & 0o777, 0o666);
  }
});

testPerm({ read: true, write: true }, async function writeFileCreate() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test.txt";
  let caughtError = false;
  // if create turned off, the file won't be created
  try {
    await Deno.writeFile(filename, data, { create: false });
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, Deno.ErrorKind.NotFound);
    assertEqual(e.name, "NotFound");
  }
  assert(caughtError);

  // Turn on create, should have no error
  await Deno.writeFile(filename, data, { create: true });
  await Deno.writeFile(filename, data, { create: false });
  const dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

testPerm({ read: true, write: true }, async function writeFileAppend() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  const filename = Deno.makeTempDirSync() + "/test.txt";
  await Deno.writeFile(filename, data);
  await Deno.writeFile(filename, data, { append: true });
  let dataRead = Deno.readFileSync(filename);
  const dec = new TextDecoder("utf-8");
  let actual = dec.decode(dataRead);
  assertEqual("HelloHello", actual);
  // Now attempt overwrite
  await Deno.writeFile(filename, data, { append: false });
  dataRead = Deno.readFileSync(filename);
  actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
  // append not set should also overwrite
  await Deno.writeFile(filename, data);
  dataRead = Deno.readFileSync(filename);
  actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});
