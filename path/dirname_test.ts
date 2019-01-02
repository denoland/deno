// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

import { test, assertEqual } from "../testing/mod.ts";
import * as path from "./index.ts";

test(function dirname() {
  assertEqual(path.posix.dirname("/a/b/"), "/a");
  assertEqual(path.posix.dirname("/a/b"), "/a");
  assertEqual(path.posix.dirname("/a"), "/");
  assertEqual(path.posix.dirname(""), ".");
  assertEqual(path.posix.dirname("/"), "/");
  assertEqual(path.posix.dirname("////"), "/");
  assertEqual(path.posix.dirname("//a"), "//");
  assertEqual(path.posix.dirname("foo"), ".");
});

test(function dirnameWin32() {
  assertEqual(path.win32.dirname("c:\\"), "c:\\");
  assertEqual(path.win32.dirname("c:\\foo"), "c:\\");
  assertEqual(path.win32.dirname("c:\\foo\\"), "c:\\");
  assertEqual(path.win32.dirname("c:\\foo\\bar"), "c:\\foo");
  assertEqual(path.win32.dirname("c:\\foo\\bar\\"), "c:\\foo");
  assertEqual(path.win32.dirname("c:\\foo\\bar\\baz"), "c:\\foo\\bar");
  assertEqual(path.win32.dirname("\\"), "\\");
  assertEqual(path.win32.dirname("\\foo"), "\\");
  assertEqual(path.win32.dirname("\\foo\\"), "\\");
  assertEqual(path.win32.dirname("\\foo\\bar"), "\\foo");
  assertEqual(path.win32.dirname("\\foo\\bar\\"), "\\foo");
  assertEqual(path.win32.dirname("\\foo\\bar\\baz"), "\\foo\\bar");
  assertEqual(path.win32.dirname("c:"), "c:");
  assertEqual(path.win32.dirname("c:foo"), "c:");
  assertEqual(path.win32.dirname("c:foo\\"), "c:");
  assertEqual(path.win32.dirname("c:foo\\bar"), "c:foo");
  assertEqual(path.win32.dirname("c:foo\\bar\\"), "c:foo");
  assertEqual(path.win32.dirname("c:foo\\bar\\baz"), "c:foo\\bar");
  assertEqual(path.win32.dirname("file:stream"), ".");
  assertEqual(path.win32.dirname("dir\\file:stream"), "dir");
  assertEqual(path.win32.dirname("\\\\unc\\share"), "\\\\unc\\share");
  assertEqual(path.win32.dirname("\\\\unc\\share\\foo"), "\\\\unc\\share\\");
  assertEqual(path.win32.dirname("\\\\unc\\share\\foo\\"), "\\\\unc\\share\\");
  assertEqual(
    path.win32.dirname("\\\\unc\\share\\foo\\bar"),
    "\\\\unc\\share\\foo"
  );
  assertEqual(
    path.win32.dirname("\\\\unc\\share\\foo\\bar\\"),
    "\\\\unc\\share\\foo"
  );
  assertEqual(
    path.win32.dirname("\\\\unc\\share\\foo\\bar\\baz"),
    "\\\\unc\\share\\foo\\bar"
  );
  assertEqual(path.win32.dirname("/a/b/"), "/a");
  assertEqual(path.win32.dirname("/a/b"), "/a");
  assertEqual(path.win32.dirname("/a"), "/");
  assertEqual(path.win32.dirname(""), ".");
  assertEqual(path.win32.dirname("/"), "/");
  assertEqual(path.win32.dirname("////"), "/");
  assertEqual(path.win32.dirname("foo"), ".");
});
