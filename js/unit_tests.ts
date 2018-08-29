// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// This test is executed as part of integration_test.go
// But it can also be run manually:
//  ./deno tests.ts

import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

import "./compiler_test.ts";
import "./console_test.ts";

test(async function tests_test() {
  assert(true);
});

test(async function tests_readFileSync() {
  const data = deno.readFileSync("package.json");
  if (!data.byteLength) {
    throw Error(
      `Expected positive value for data.byteLength ${data.byteLength}`
    );
  }
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEqual(pkg.name, "deno");
});

/* TODO We should be able to catch specific types.
test(function tests_readFileSync_NotFound() {
  let caughtError = false;
  let data;
  try {
    data = deno.readFileSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assert(e instanceof deno.NotFound);
  }
  assert(caughtError);
  assert(data === undefined);
});
*/

/* TODO(ry) Add this once we can create a tmpDir to write the file into.
test(function writeFileSyncSuccess() {
  const enc = new TextEncoder();
  const dataWritten = enc.encode("Hello");
  const filename = "TEMPDIR/test.txt";
  deno.writeFileSync(filename, dataWritten, 0o666);
  const dataRead = deno.readFileSync(filename);
  assertEqual(dataRead, dataWritten);
});
*/

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
    // TODO assertEqual(e, deno.NotFound);
    assertEqual(e.name, "deno.NotFound");
  }
  assert(caughtError);
});

testPerm({ net: true }, async function tests_fetch() {
  const response = await fetch("http://localhost:4545/package.json");
  const json = await response.json();
  assertEqual(json.name, "deno");
});

/*
test(async function tests_writeFileSync() {
  const enc = new TextEncoder();
  const data = enc.encode("Hello");
  // TODO need ability to get tmp dir.
  const fn = "/tmp/test.txt";
  writeFileSync("/tmp/test.txt", data, 0o666);
  const dataRead = deno.readFileSync("/tmp/test.txt");
  const dec = new TextDecoder("utf-8");
  const actual = dec.decode(dataRead);
  assertEqual("Hello", actual);
});

*/
