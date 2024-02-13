// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals } from "../assert/mod.ts";
import * as path from "./mod.ts";

// Test suite from "GNU core utilities"
// https://github.com/coreutils/coreutils/blob/master/tests/misc/dirname.pl
const COREUTILS_TESTSUITE = [
  ["d/f", "d"],
  ["/d/f", "/d"],
  ["d/f/", "d"],
  ["d/f//", "d"],
  ["f", "."],
  ["/", "/"],
  ["//", "/"],
  ["///", "/"],
  ["//a//", "/"],
  ["///a///", "/"],
  ["///a///b", "///a"],
  ["///a//b/", "///a"],
  ["", "."],
];

const POSIX_TESTSUITE = [
  ["/a/b/", "/a"],
  ["/a/b", "/a"],
  ["/a", "/"],
  ["", "."],
  ["/", "/"],
  ["////", "/"],
  ["//a", "/"],
  ["foo", "."],
];

const WIN32_TESTSUITE = [
  ["c:\\", "c:\\"],
  ["c:\\foo", "c:\\"],
  ["c:\\foo\\", "c:\\"],
  ["c:\\foo\\bar", "c:\\foo"],
  ["c:\\foo\\bar\\", "c:\\foo"],
  ["c:\\foo\\bar\\baz", "c:\\foo\\bar"],
  ["\\", "\\"],
  ["\\foo", "\\"],
  ["\\foo\\", "\\"],
  ["\\foo\\bar", "\\foo"],
  ["\\foo\\bar\\", "\\foo"],
  ["\\foo\\bar\\baz", "\\foo\\bar"],
  ["c:", "c:"],
  ["c:foo", "c:"],
  ["c:foo\\", "c:"],
  ["c:foo\\bar", "c:foo"],
  ["c:foo\\bar\\", "c:foo"],
  ["c:foo\\bar\\baz", "c:foo\\bar"],
  ["file:stream", "."],
  ["dir\\file:stream", "dir"],
  ["\\\\unc\\share", "\\\\unc\\share"],
  ["\\\\unc\\share\\foo", "\\\\unc\\share\\"],
  ["\\\\unc\\share\\foo\\", "\\\\unc\\share\\"],
  ["\\\\unc\\share\\foo\\bar", "\\\\unc\\share\\foo"],
  ["\\\\unc\\share\\foo\\bar\\", "\\\\unc\\share\\foo"],
  ["\\\\unc\\share\\foo\\bar\\baz", "\\\\unc\\share\\foo\\bar"],
  ["/a/b/", "/a"],
  ["/a/b", "/a"],
  ["/a", "/"],
  ["", "."],
  ["/", "/"],
  ["////", "/"],
  ["foo", "."],
];

Deno.test("posix.dirname()", function () {
  for (const [name, expected] of COREUTILS_TESTSUITE) {
    assertEquals(path.dirname(name), expected);
  }

  for (const [name, expected] of POSIX_TESTSUITE) {
    assertEquals(path.posix.dirname(name), expected);
  }

  // POSIX treats backslash as any other character.
  assertEquals(path.posix.dirname("\\foo/bar"), "\\foo");
  assertEquals(path.posix.dirname("\\/foo/bar"), "\\/foo");
  assertEquals(path.posix.dirname("/foo/bar\\baz/qux"), "/foo/bar\\baz");
  assertEquals(path.posix.dirname("/foo/bar/baz\\"), "/foo/bar");
});

Deno.test("win32.dirname()", function () {
  for (const [name, expected] of WIN32_TESTSUITE) {
    assertEquals(path.win32.dirname(name), expected);
  }

  // path.win32 should pass all "forward slash" posix tests as well.
  for (const [name, expected] of COREUTILS_TESTSUITE) {
    assertEquals(path.win32.dirname(name), expected);
  }

  for (const [name, expected] of POSIX_TESTSUITE) {
    assertEquals(path.win32.dirname(name), expected);
  }
});
