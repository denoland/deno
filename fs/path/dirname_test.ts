// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import * as path from "./mod.ts";

test(function dirname() {
  assertEq(path.posix.dirname("/a/b/"), "/a");
  assertEq(path.posix.dirname("/a/b"), "/a");
  assertEq(path.posix.dirname("/a"), "/");
  assertEq(path.posix.dirname(""), ".");
  assertEq(path.posix.dirname("/"), "/");
  assertEq(path.posix.dirname("////"), "/");
  assertEq(path.posix.dirname("//a"), "//");
  assertEq(path.posix.dirname("foo"), ".");
});

test(function dirnameWin32() {
  assertEq(path.win32.dirname("c:\\"), "c:\\");
  assertEq(path.win32.dirname("c:\\foo"), "c:\\");
  assertEq(path.win32.dirname("c:\\foo\\"), "c:\\");
  assertEq(path.win32.dirname("c:\\foo\\bar"), "c:\\foo");
  assertEq(path.win32.dirname("c:\\foo\\bar\\"), "c:\\foo");
  assertEq(path.win32.dirname("c:\\foo\\bar\\baz"), "c:\\foo\\bar");
  assertEq(path.win32.dirname("\\"), "\\");
  assertEq(path.win32.dirname("\\foo"), "\\");
  assertEq(path.win32.dirname("\\foo\\"), "\\");
  assertEq(path.win32.dirname("\\foo\\bar"), "\\foo");
  assertEq(path.win32.dirname("\\foo\\bar\\"), "\\foo");
  assertEq(path.win32.dirname("\\foo\\bar\\baz"), "\\foo\\bar");
  assertEq(path.win32.dirname("c:"), "c:");
  assertEq(path.win32.dirname("c:foo"), "c:");
  assertEq(path.win32.dirname("c:foo\\"), "c:");
  assertEq(path.win32.dirname("c:foo\\bar"), "c:foo");
  assertEq(path.win32.dirname("c:foo\\bar\\"), "c:foo");
  assertEq(path.win32.dirname("c:foo\\bar\\baz"), "c:foo\\bar");
  assertEq(path.win32.dirname("file:stream"), ".");
  assertEq(path.win32.dirname("dir\\file:stream"), "dir");
  assertEq(path.win32.dirname("\\\\unc\\share"), "\\\\unc\\share");
  assertEq(path.win32.dirname("\\\\unc\\share\\foo"), "\\\\unc\\share\\");
  assertEq(path.win32.dirname("\\\\unc\\share\\foo\\"), "\\\\unc\\share\\");
  assertEq(
    path.win32.dirname("\\\\unc\\share\\foo\\bar"),
    "\\\\unc\\share\\foo"
  );
  assertEq(
    path.win32.dirname("\\\\unc\\share\\foo\\bar\\"),
    "\\\\unc\\share\\foo"
  );
  assertEq(
    path.win32.dirname("\\\\unc\\share\\foo\\bar\\baz"),
    "\\\\unc\\share\\foo\\bar"
  );
  assertEq(path.win32.dirname("/a/b/"), "/a");
  assertEq(path.win32.dirname("/a/b"), "/a");
  assertEq(path.win32.dirname("/a"), "/");
  assertEq(path.win32.dirname(""), ".");
  assertEq(path.win32.dirname("/"), "/");
  assertEq(path.win32.dirname("////"), "/");
  assertEq(path.win32.dirname("foo"), ".");
});
