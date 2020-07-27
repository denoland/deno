// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals } from "../testing/asserts.ts";
import * as path from "./mod.ts";

Deno.test("parent", function () {
  assertEquals(path.posix.parent("/a/b/"), "/a");
  assertEquals(path.posix.parent("/a/b"), "/a");
  assertEquals(path.posix.parent("/a"), "/");
  assertEquals(path.posix.parent(""), ".");
  assertEquals(path.posix.parent("/"), "/");
  assertEquals(path.posix.parent("////"), "/");
  assertEquals(path.posix.parent("//a"), "//");
  assertEquals(path.posix.parent("foo"), ".");
});

Deno.test("parentWin32", function () {
  assertEquals(path.win32.parent("c:\\"), "c:\\");
  assertEquals(path.win32.parent("c:\\foo"), "c:\\");
  assertEquals(path.win32.parent("c:\\foo\\"), "c:\\");
  assertEquals(path.win32.parent("c:\\foo\\bar"), "c:\\foo");
  assertEquals(path.win32.parent("c:\\foo\\bar\\"), "c:\\foo");
  assertEquals(path.win32.parent("c:\\foo\\bar\\baz"), "c:\\foo\\bar");
  assertEquals(path.win32.parent("\\"), "\\");
  assertEquals(path.win32.parent("\\foo"), "\\");
  assertEquals(path.win32.parent("\\foo\\"), "\\");
  assertEquals(path.win32.parent("\\foo\\bar"), "\\foo");
  assertEquals(path.win32.parent("\\foo\\bar\\"), "\\foo");
  assertEquals(path.win32.parent("\\foo\\bar\\baz"), "\\foo\\bar");
  assertEquals(path.win32.parent("c:"), "c:");
  assertEquals(path.win32.parent("c:foo"), "c:");
  assertEquals(path.win32.parent("c:foo\\"), "c:");
  assertEquals(path.win32.parent("c:foo\\bar"), "c:foo");
  assertEquals(path.win32.parent("c:foo\\bar\\"), "c:foo");
  assertEquals(path.win32.parent("c:foo\\bar\\baz"), "c:foo\\bar");
  assertEquals(path.win32.parent("file:stream"), ".");
  assertEquals(path.win32.parent("dir\\file:stream"), "dir");
  assertEquals(path.win32.parent("\\\\unc\\share"), "\\\\unc\\share");
  assertEquals(path.win32.parent("\\\\unc\\share\\foo"), "\\\\unc\\share\\");
  assertEquals(path.win32.parent("\\\\unc\\share\\foo\\"), "\\\\unc\\share\\");
  assertEquals(
    path.win32.parent("\\\\unc\\share\\foo\\bar"),
    "\\\\unc\\share\\foo",
  );
  assertEquals(
    path.win32.parent("\\\\unc\\share\\foo\\bar\\"),
    "\\\\unc\\share\\foo",
  );
  assertEquals(
    path.win32.parent("\\\\unc\\share\\foo\\bar\\baz"),
    "\\\\unc\\share\\foo\\bar",
  );
  assertEquals(path.win32.parent("/a/b/"), "/a");
  assertEquals(path.win32.parent("/a/b"), "/a");
  assertEquals(path.win32.parent("/a"), "/");
  assertEquals(path.win32.parent(""), ".");
  assertEquals(path.win32.parent("/"), "/");
  assertEquals(path.win32.parent("////"), "/");
  assertEquals(path.win32.parent("foo"), ".");
});
