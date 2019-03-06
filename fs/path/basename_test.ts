// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import * as path from "./mod.ts";

test(function basename() {
  assertEq(path.basename(".js", ".js"), "");
  assertEq(path.basename(""), "");
  assertEq(path.basename("/dir/basename.ext"), "basename.ext");
  assertEq(path.basename("/basename.ext"), "basename.ext");
  assertEq(path.basename("basename.ext"), "basename.ext");
  assertEq(path.basename("basename.ext/"), "basename.ext");
  assertEq(path.basename("basename.ext//"), "basename.ext");
  assertEq(path.basename("aaa/bbb", "/bbb"), "bbb");
  assertEq(path.basename("aaa/bbb", "a/bbb"), "bbb");
  assertEq(path.basename("aaa/bbb", "bbb"), "bbb");
  assertEq(path.basename("aaa/bbb//", "bbb"), "bbb");
  assertEq(path.basename("aaa/bbb", "bb"), "b");
  assertEq(path.basename("aaa/bbb", "b"), "bb");
  assertEq(path.basename("/aaa/bbb", "/bbb"), "bbb");
  assertEq(path.basename("/aaa/bbb", "a/bbb"), "bbb");
  assertEq(path.basename("/aaa/bbb", "bbb"), "bbb");
  assertEq(path.basename("/aaa/bbb//", "bbb"), "bbb");
  assertEq(path.basename("/aaa/bbb", "bb"), "b");
  assertEq(path.basename("/aaa/bbb", "b"), "bb");
  assertEq(path.basename("/aaa/bbb"), "bbb");
  assertEq(path.basename("/aaa/"), "aaa");
  assertEq(path.basename("/aaa/b"), "b");
  assertEq(path.basename("/a/b"), "b");
  assertEq(path.basename("//a"), "a");

  // On unix a backslash is just treated as any other character.
  assertEq(path.posix.basename("\\dir\\basename.ext"), "\\dir\\basename.ext");
  assertEq(path.posix.basename("\\basename.ext"), "\\basename.ext");
  assertEq(path.posix.basename("basename.ext"), "basename.ext");
  assertEq(path.posix.basename("basename.ext\\"), "basename.ext\\");
  assertEq(path.posix.basename("basename.ext\\\\"), "basename.ext\\\\");
  assertEq(path.posix.basename("foo"), "foo");

  // POSIX filenames may include control characters
  const controlCharFilename = "Icon" + String.fromCharCode(13);
  assertEq(
    path.posix.basename("/a/b/" + controlCharFilename),
    controlCharFilename
  );
});

test(function basenameWin32() {
  assertEq(path.win32.basename("\\dir\\basename.ext"), "basename.ext");
  assertEq(path.win32.basename("\\basename.ext"), "basename.ext");
  assertEq(path.win32.basename("basename.ext"), "basename.ext");
  assertEq(path.win32.basename("basename.ext\\"), "basename.ext");
  assertEq(path.win32.basename("basename.ext\\\\"), "basename.ext");
  assertEq(path.win32.basename("foo"), "foo");
  assertEq(path.win32.basename("aaa\\bbb", "\\bbb"), "bbb");
  assertEq(path.win32.basename("aaa\\bbb", "a\\bbb"), "bbb");
  assertEq(path.win32.basename("aaa\\bbb", "bbb"), "bbb");
  assertEq(path.win32.basename("aaa\\bbb\\\\\\\\", "bbb"), "bbb");
  assertEq(path.win32.basename("aaa\\bbb", "bb"), "b");
  assertEq(path.win32.basename("aaa\\bbb", "b"), "bb");
  assertEq(path.win32.basename("C:"), "");
  assertEq(path.win32.basename("C:."), ".");
  assertEq(path.win32.basename("C:\\"), "");
  assertEq(path.win32.basename("C:\\dir\\base.ext"), "base.ext");
  assertEq(path.win32.basename("C:\\basename.ext"), "basename.ext");
  assertEq(path.win32.basename("C:basename.ext"), "basename.ext");
  assertEq(path.win32.basename("C:basename.ext\\"), "basename.ext");
  assertEq(path.win32.basename("C:basename.ext\\\\"), "basename.ext");
  assertEq(path.win32.basename("C:foo"), "foo");
  assertEq(path.win32.basename("file:stream"), "file:stream");
});
