// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ write: true }, function makeTempDirSyncSuccess() {
  const dir1 = deno.makeTempDirSync({ prefix: "hello", suffix: "world" });
  const dir2 = deno.makeTempDirSync({ prefix: "hello", suffix: "world" });
  // Check that both dirs are different.
  assert(dir1 !== dir2);
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

testPerm({ write: true }, async function makeTempDirSuccess() {
  const dir1 = await deno.makeTempDir({ prefix: "hello", suffix: "world" });
  const dir2 = await deno.makeTempDir({ prefix: "hello", suffix: "world" });
  // Check that both dirs are different.
  assert(dir1 !== dir2);
  for (const dir of [dir1, dir2]) {
    // Check that the prefix and suffix are applied.
    const lastPart = dir.replace(/^.*[\\\/]/, "");
    assert(lastPart.startsWith("hello"));
    assert(lastPart.endsWith("world"));
  }
  // Check that the `dir` option works.
  const dir3 = await deno.makeTempDir({ dir: dir1 });
  assert(dir3.startsWith(dir1));
  assert(/^[\\\/]/.test(dir3.slice(dir1.length)));
  // Check that creating a temp dir inside a nonexisting directory fails.
  let err;
  try {
    await deno.makeTempDir({ dir: "/baddir" });
  } catch (err_) {
    err = err_;
  }
  assertEqual(err.kind, deno.ErrorKind.NotFound);
  assertEqual(err.name, "NotFound");
});
