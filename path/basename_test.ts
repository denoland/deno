// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import * as path from "./index";

test(function basename() {
  assertEqual(path.basename(".js", ".js"), "");
  assertEqual(path.basename(""), "");
  assertEqual(path.basename("/dir/basename.ext"), "basename.ext");
  assertEqual(path.basename("/basename.ext"), "basename.ext");
  assertEqual(path.basename("basename.ext"), "basename.ext");
  assertEqual(path.basename("basename.ext/"), "basename.ext");
  assertEqual(path.basename("basename.ext//"), "basename.ext");
  assertEqual(path.basename("aaa/bbb", "/bbb"), "bbb");
  assertEqual(path.basename("aaa/bbb", "a/bbb"), "bbb");
  assertEqual(path.basename("aaa/bbb", "bbb"), "bbb");
  assertEqual(path.basename("aaa/bbb//", "bbb"), "bbb");
  assertEqual(path.basename("aaa/bbb", "bb"), "b");
  assertEqual(path.basename("aaa/bbb", "b"), "bb");
  assertEqual(path.basename("/aaa/bbb", "/bbb"), "bbb");
  assertEqual(path.basename("/aaa/bbb", "a/bbb"), "bbb");
  assertEqual(path.basename("/aaa/bbb", "bbb"), "bbb");
  assertEqual(path.basename("/aaa/bbb//", "bbb"), "bbb");
  assertEqual(path.basename("/aaa/bbb", "bb"), "b");
  assertEqual(path.basename("/aaa/bbb", "b"), "bb");
  assertEqual(path.basename("/aaa/bbb"), "bbb");
  assertEqual(path.basename("/aaa/"), "aaa");
  assertEqual(path.basename("/aaa/b"), "b");
  assertEqual(path.basename("/a/b"), "b");
  assertEqual(path.basename("//a"), "a");

  // On unix a backslash is just treated as any other character.
  assertEqual(
    path.posix.basename("\\dir\\basename.ext"),
    "\\dir\\basename.ext"
  );
  assertEqual(path.posix.basename("\\basename.ext"), "\\basename.ext");
  assertEqual(path.posix.basename("basename.ext"), "basename.ext");
  assertEqual(path.posix.basename("basename.ext\\"), "basename.ext\\");
  assertEqual(path.posix.basename("basename.ext\\\\"), "basename.ext\\\\");
  assertEqual(path.posix.basename("foo"), "foo");

  // POSIX filenames may include control characters
  const controlCharFilename = "Icon" + String.fromCharCode(13);
  assertEqual(
    path.posix.basename("/a/b/" + controlCharFilename),
    controlCharFilename
  );
});

test(function basenameWin32() {
  assertEqual(path.win32.basename("\\dir\\basename.ext"), "basename.ext");
  assertEqual(path.win32.basename("\\basename.ext"), "basename.ext");
  assertEqual(path.win32.basename("basename.ext"), "basename.ext");
  assertEqual(path.win32.basename("basename.ext\\"), "basename.ext");
  assertEqual(path.win32.basename("basename.ext\\\\"), "basename.ext");
  assertEqual(path.win32.basename("foo"), "foo");
  assertEqual(path.win32.basename("aaa\\bbb", "\\bbb"), "bbb");
  assertEqual(path.win32.basename("aaa\\bbb", "a\\bbb"), "bbb");
  assertEqual(path.win32.basename("aaa\\bbb", "bbb"), "bbb");
  assertEqual(path.win32.basename("aaa\\bbb\\\\\\\\", "bbb"), "bbb");
  assertEqual(path.win32.basename("aaa\\bbb", "bb"), "b");
  assertEqual(path.win32.basename("aaa\\bbb", "b"), "bb");
  assertEqual(path.win32.basename("C:"), "");
  assertEqual(path.win32.basename("C:."), ".");
  assertEqual(path.win32.basename("C:\\"), "");
  assertEqual(path.win32.basename("C:\\dir\\base.ext"), "base.ext");
  assertEqual(path.win32.basename("C:\\basename.ext"), "basename.ext");
  assertEqual(path.win32.basename("C:basename.ext"), "basename.ext");
  assertEqual(path.win32.basename("C:basename.ext\\"), "basename.ext");
  assertEqual(path.win32.basename("C:basename.ext\\\\"), "basename.ext");
  assertEqual(path.win32.basename("C:foo"), "foo");
  assertEqual(path.win32.basename("file:stream"), "file:stream");
});
