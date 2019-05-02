// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEquals } from "./test_util.ts";

testPerm({ write: true }, function makeTempDirSyncSuccess(): void {
  const dir1 = Deno.makeTempDirSync({ prefix: "hello", suffix: "world" });
  const dir2 = Deno.makeTempDirSync({ prefix: "hello", suffix: "world" });
  // Check that both dirs are different.
  assert(dir1 !== dir2);
  for (const dir of [dir1, dir2]) {
    // Check that the prefix and suffix are applied.
    const lastPart = dir.replace(/^.*[\\\/]/, "");
    assert(lastPart.startsWith("hello"));
    assert(lastPart.endsWith("world"));
  }
  // Check that the `dir` option works.
  const dir3 = Deno.makeTempDirSync({ dir: dir1 });
  assert(dir3.startsWith(dir1));
  assert(/^[\\\/]/.test(dir3.slice(dir1.length)));
  // Check that creating a temp dir inside a nonexisting directory fails.
  let err;
  try {
    Deno.makeTempDirSync({ dir: "/baddir" });
  } catch (err_) {
    err = err_;
  }
  assertEquals(err.kind, Deno.ErrorKind.NotFound);
  assertEquals(err.name, "NotFound");
});

test(function makeTempDirSyncPerm(): void {
  // makeTempDirSync should require write permissions (for now).
  let err;
  try {
    Deno.makeTempDirSync({ dir: "/baddir" });
  } catch (err_) {
    err = err_;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ write: true }, async function makeTempDirSuccess(): Promise<void> {
  const dir1 = await Deno.makeTempDir({ prefix: "hello", suffix: "world" });
  const dir2 = await Deno.makeTempDir({ prefix: "hello", suffix: "world" });
  // Check that both dirs are different.
  assert(dir1 !== dir2);
  for (const dir of [dir1, dir2]) {
    // Check that the prefix and suffix are applied.
    const lastPart = dir.replace(/^.*[\\\/]/, "");
    assert(lastPart.startsWith("hello"));
    assert(lastPart.endsWith("world"));
  }
  // Check that the `dir` option works.
  const dir3 = await Deno.makeTempDir({ dir: dir1 });
  assert(dir3.startsWith(dir1));
  assert(/^[\\\/]/.test(dir3.slice(dir1.length)));
  // Check that creating a temp dir inside a nonexisting directory fails.
  let err;
  try {
    await Deno.makeTempDir({ dir: "/baddir" });
  } catch (err_) {
    err = err_;
  }
  assertEquals(err.kind, Deno.ErrorKind.NotFound);
  assertEquals(err.name, "NotFound");
});
