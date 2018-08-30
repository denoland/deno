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

testPerm({ write: true }, function writeFileSync() {
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
    // TODO assertEqual(e, deno.NotFound);
    assertEqual(e.name, "deno.NotFound");
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
  // TODO assert(err instanceof deno.NotFound).
  assert(err);
  assertEqual(err.name, "deno.NotFound");
});

test(function makeTempDirSyncPerm() {
  // makeTempDirSync should require write permissions (for now).
  let err;
  try {
    deno.makeTempDirSync({ dir: "/baddir" });
  } catch (err_) {
    err = err_;
  }
  // TODO assert(err instanceof deno.PermissionDenied).
  assert(err);
  assertEqual(err.name, "deno.PermissionDenied");
});

testPerm({ net: true }, async function tests_fetch() {
  const response = await fetch("http://localhost:4545/package.json");
  const json = await response.json();
  assertEqual(json.name, "deno");
});
