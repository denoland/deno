// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals } from "../testing/asserts.ts";
import * as path from "./mod.ts";

Deno.test("basename", function () {
  assertEquals(path.basename(".js", ".js"), "");
  assertEquals(path.basename(""), "");
  assertEquals(path.basename("/dir/basename.ext"), "basename.ext");
  assertEquals(path.basename("/basename.ext"), "basename.ext");
  assertEquals(path.basename("basename.ext"), "basename.ext");
  assertEquals(path.basename("basename.ext/"), "basename.ext");
  assertEquals(path.basename("basename.ext//"), "basename.ext");
  assertEquals(path.basename("aaa/bbb", "/bbb"), "bbb");
  assertEquals(path.basename("aaa/bbb", "a/bbb"), "bbb");
  assertEquals(path.basename("aaa/bbb", "bbb"), "bbb");
  assertEquals(path.basename("aaa/bbb//", "bbb"), "bbb");
  assertEquals(path.basename("aaa/bbb", "bb"), "b");
  assertEquals(path.basename("aaa/bbb", "b"), "bb");
  assertEquals(path.basename("/aaa/bbb", "/bbb"), "bbb");
  assertEquals(path.basename("/aaa/bbb", "a/bbb"), "bbb");
  assertEquals(path.basename("/aaa/bbb", "bbb"), "bbb");
  assertEquals(path.basename("/aaa/bbb//", "bbb"), "bbb");
  assertEquals(path.basename("/aaa/bbb", "bb"), "b");
  assertEquals(path.basename("/aaa/bbb", "b"), "bb");
  assertEquals(path.basename("/aaa/bbb"), "bbb");
  assertEquals(path.basename("/aaa/"), "aaa");
  assertEquals(path.basename("/aaa/b"), "b");
  assertEquals(path.basename("/a/b"), "b");
  assertEquals(path.basename("//a"), "a");

  // On unix a backslash is just treated as any other character.
  assertEquals(
    path.posix.basename("\\dir\\basename.ext"),
    "\\dir\\basename.ext"
  );
  assertEquals(path.posix.basename("\\basename.ext"), "\\basename.ext");
  assertEquals(path.posix.basename("basename.ext"), "basename.ext");
  assertEquals(path.posix.basename("basename.ext\\"), "basename.ext\\");
  assertEquals(path.posix.basename("basename.ext\\\\"), "basename.ext\\\\");
  assertEquals(path.posix.basename("foo"), "foo");

  // POSIX filenames may include control characters
  const controlCharFilename = "Icon" + String.fromCharCode(13);
  assertEquals(
    path.posix.basename("/a/b/" + controlCharFilename),
    controlCharFilename
  );
});

Deno.test("basenameWin32", function () {
  assertEquals(path.win32.basename("\\dir\\basename.ext"), "basename.ext");
  assertEquals(path.win32.basename("\\basename.ext"), "basename.ext");
  assertEquals(path.win32.basename("basename.ext"), "basename.ext");
  assertEquals(path.win32.basename("basename.ext\\"), "basename.ext");
  assertEquals(path.win32.basename("basename.ext\\\\"), "basename.ext");
  assertEquals(path.win32.basename("foo"), "foo");
  assertEquals(path.win32.basename("aaa\\bbb", "\\bbb"), "bbb");
  assertEquals(path.win32.basename("aaa\\bbb", "a\\bbb"), "bbb");
  assertEquals(path.win32.basename("aaa\\bbb", "bbb"), "bbb");
  assertEquals(path.win32.basename("aaa\\bbb\\\\\\\\", "bbb"), "bbb");
  assertEquals(path.win32.basename("aaa\\bbb", "bb"), "b");
  assertEquals(path.win32.basename("aaa\\bbb", "b"), "bb");
  assertEquals(path.win32.basename("C:"), "");
  assertEquals(path.win32.basename("C:."), ".");
  assertEquals(path.win32.basename("C:\\"), "");
  assertEquals(path.win32.basename("C:\\dir\\base.ext"), "base.ext");
  assertEquals(path.win32.basename("C:\\basename.ext"), "basename.ext");
  assertEquals(path.win32.basename("C:basename.ext"), "basename.ext");
  assertEquals(path.win32.basename("C:basename.ext\\"), "basename.ext");
  assertEquals(path.win32.basename("C:basename.ext\\\\"), "basename.ext");
  assertEquals(path.win32.basename("C:foo"), "foo");
  assertEquals(path.win32.basename("file:stream"), "file:stream");
});
