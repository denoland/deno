// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as deno from "deno";
import { assert, testPerm, assertEqual } from "../js/test_util.ts";
import { findFiles } from "./util.ts";

const testDir = "tools/testdata/find_files_testdata";

// Sorts and replace backslashes with slashes.
const normalize = files => files.map(f => f.replace(/\\/g, "/")).sort();

testPerm({ read: true }, function testFindFiles() {
  const files = findFiles([testDir], [".ts", ".md"]);
  assertEqual(normalize(files), [
    `${testDir}/bar.md`,
    `${testDir}/bar.ts`,
    `${testDir}/foo.md`,
    `${testDir}/foo.ts`,
    `${testDir}/subdir0/bar.ts`,
    `${testDir}/subdir0/foo.ts`,
    `${testDir}/subdir0/subdir0/bar.ts`,
    `${testDir}/subdir0/subdir0/foo.ts`,
    `${testDir}/subdir1/bar.ts`,
    `${testDir}/subdir1/foo.ts`
  ]);
});

testPerm({ read: true }, function testFindFilesDepth() {
  const files = findFiles([testDir], [".ts", ".md"], { depth: 1 });
  assertEqual(normalize(files), [
    `${testDir}/bar.md`,
    `${testDir}/bar.ts`,
    `${testDir}/foo.md`,
    `${testDir}/foo.ts`
  ]);
});

testPerm({ read: true }, function testFindFilesSkip() {
  const files = findFiles([testDir], [".ts", ".md"], {
    skip: ["foo.md", "subdir1"]
  });
  assertEqual(normalize(files), [
    `${testDir}/bar.md`,
    `${testDir}/bar.ts`,
    `${testDir}/foo.ts`,
    `${testDir}/subdir0/bar.ts`,
    `${testDir}/subdir0/foo.ts`,
    `${testDir}/subdir0/subdir0/bar.ts`,
    `${testDir}/subdir0/subdir0/foo.ts`
  ]);
});

testPerm({ read: false }, function testFindFilesPerm() {
  let caughtError = false;
  try {
    const files = findFiles([testDir], [".ts", ".md"]);
  } catch (e) {
    caughtError = true;
    assertEqual(e.kind, deno.ErrorKind.PermissionDenied);
    assertEqual(e.name, "PermissionDenied");
  }
  assert(caughtError);
});
