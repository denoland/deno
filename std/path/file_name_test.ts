// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals } from "../testing/asserts.ts";
import * as path from "./mod.ts";

Deno.test("fileName", function () {
  assertEquals(path.fileName(".js", ".js"), "");
  assertEquals(path.fileName(""), "");
  assertEquals(path.fileName("/dir/file_name.ext"), "file_name.ext");
  assertEquals(path.fileName("/file_name.ext"), "file_name.ext");
  assertEquals(path.fileName("file_name.ext"), "file_name.ext");
  assertEquals(path.fileName("file_name.ext/"), "file_name.ext");
  assertEquals(path.fileName("file_name.ext//"), "file_name.ext");
  assertEquals(path.fileName("aaa/bbb", "/bbb"), "bbb");
  assertEquals(path.fileName("aaa/bbb", "a/bbb"), "bbb");
  assertEquals(path.fileName("aaa/bbb", "bbb"), "bbb");
  assertEquals(path.fileName("aaa/bbb//", "bbb"), "bbb");
  assertEquals(path.fileName("aaa/bbb", "bb"), "b");
  assertEquals(path.fileName("aaa/bbb", "b"), "bb");
  assertEquals(path.fileName("/aaa/bbb", "/bbb"), "bbb");
  assertEquals(path.fileName("/aaa/bbb", "a/bbb"), "bbb");
  assertEquals(path.fileName("/aaa/bbb", "bbb"), "bbb");
  assertEquals(path.fileName("/aaa/bbb//", "bbb"), "bbb");
  assertEquals(path.fileName("/aaa/bbb", "bb"), "b");
  assertEquals(path.fileName("/aaa/bbb", "b"), "bb");
  assertEquals(path.fileName("/aaa/bbb"), "bbb");
  assertEquals(path.fileName("/aaa/"), "aaa");
  assertEquals(path.fileName("/aaa/b"), "b");
  assertEquals(path.fileName("/a/b"), "b");
  assertEquals(path.fileName("//a"), "a");

  // On unix a backslash is just treated as any other character.
  assertEquals(
    path.posix.fileName("\\dir\\file_name.ext"),
    "\\dir\\file_name.ext",
  );
  assertEquals(path.posix.fileName("\\file_name.ext"), "\\file_name.ext");
  assertEquals(path.posix.fileName("file_name.ext"), "file_name.ext");
  assertEquals(path.posix.fileName("file_name.ext\\"), "file_name.ext\\");
  assertEquals(path.posix.fileName("file_name.ext\\\\"), "file_name.ext\\\\");
  assertEquals(path.posix.fileName("foo"), "foo");

  // POSIX filenames may include control characters
  const controlCharFilename = "Icon" + String.fromCharCode(13);
  assertEquals(
    path.posix.fileName("/a/b/" + controlCharFilename),
    controlCharFilename,
  );
});

Deno.test("fileNameWin32", function () {
  assertEquals(path.win32.fileName("\\dir\\file_name.ext"), "file_name.ext");
  assertEquals(path.win32.fileName("\\file_name.ext"), "file_name.ext");
  assertEquals(path.win32.fileName("file_name.ext"), "file_name.ext");
  assertEquals(path.win32.fileName("file_name.ext\\"), "file_name.ext");
  assertEquals(path.win32.fileName("file_name.ext\\\\"), "file_name.ext");
  assertEquals(path.win32.fileName("foo"), "foo");
  assertEquals(path.win32.fileName("aaa\\bbb", "\\bbb"), "bbb");
  assertEquals(path.win32.fileName("aaa\\bbb", "a\\bbb"), "bbb");
  assertEquals(path.win32.fileName("aaa\\bbb", "bbb"), "bbb");
  assertEquals(path.win32.fileName("aaa\\bbb\\\\\\\\", "bbb"), "bbb");
  assertEquals(path.win32.fileName("aaa\\bbb", "bb"), "b");
  assertEquals(path.win32.fileName("aaa\\bbb", "b"), "bb");
  assertEquals(path.win32.fileName("C:"), "");
  assertEquals(path.win32.fileName("C:."), ".");
  assertEquals(path.win32.fileName("C:\\"), "");
  assertEquals(path.win32.fileName("C:\\dir\\base.ext"), "base.ext");
  assertEquals(path.win32.fileName("C:\\file_name.ext"), "file_name.ext");
  assertEquals(path.win32.fileName("C:file_name.ext"), "file_name.ext");
  assertEquals(path.win32.fileName("C:file_name.ext\\"), "file_name.ext");
  assertEquals(path.win32.fileName("C:file_name.ext\\\\"), "file_name.ext");
  assertEquals(path.win32.fileName("C:foo"), "foo");
  assertEquals(path.win32.fileName("file:stream"), "file:stream");
});
