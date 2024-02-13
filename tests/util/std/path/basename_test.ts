// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals } from "../assert/mod.ts";
import * as path from "./mod.ts";

// Test suite from "GNU core utilities"
// https://github.com/coreutils/coreutils/blob/master/tests/misc/basename.pl
const COREUTILS_TESTSUITE = [
  [["d/f"], "f"],
  [["/d/f"], "f"],
  [["d/f/"], "f"],
  [["d/f//"], "f"],
  [["f"], "f"],
  [["/"], "/"],
  [["///"], "/"],
  [["///a///"], "a"],
  [[""], ""],
  [["aa", "a"], "a"],
  [["a-a", "-a"], "a"],
  [["f.s", ".s"], "f"],
  [["fs", "s"], "f"],
  [["fs", "fs"], "fs"],
  [["fs/", "s"], "f"],
  [["dir/file.suf", ".suf"], "file"],
  [["fs", "x"], "fs"],
  [["fs", ""], "fs"],
  [["fs/", "s/"], "fs"],
];

const POSIX_TESTSUITE = [
  [[""], ""],
  [["/dir/basename.ext"], "basename.ext"],
  [["/basename.ext"], "basename.ext"],
  [["basename.ext"], "basename.ext"],
  [["basename.ext/"], "basename.ext"],
  [["basename.ext//"], "basename.ext"],
  [["aaa/bbb", "/bbb"], "bbb"],
  [["aaa/bbb", "a/bbb"], "bbb"],
  [["aaa/bbb", "bbb"], "bbb"],
  [["aaa/bbb//", "bbb"], "bbb"],
  [["aaa/bbb", "bb"], "b"],
  [["aaa/bbb", "b"], "bb"],
  [["/aaa/bbb", "/bbb"], "bbb"],
  [["/aaa/bbb", "a/bbb"], "bbb"],
  [["/aaa/bbb", "bbb"], "bbb"],
  [["/aaa/bbb//", "bbb"], "bbb"],
  [["/aaa/bbb//", "a/bbb"], "bbb"],
  [["/aaa/bbb", "bb"], "b"],
  [["/aaa/bbb", "b"], "bb"],
  [["/aaa/bbb"], "bbb"],
  [["/aaa/"], "aaa"],
  [["/aaa/b"], "b"],
  [["/a/b"], "b"],
  [["//a"], "a"],
  [["///"], "/"],
  [["///", "bbb"], "/"],
  [["//", "bbb"], "/"],
];

const WIN32_TESTSUITE = [
  [["\\dir\\basename.ext"], "basename.ext"],
  [["\\basename.ext"], "basename.ext"],
  [["basename.ext"], "basename.ext"],
  [["basename.ext\\"], "basename.ext"],
  [["basename.ext\\\\"], "basename.ext"],
  [["foo"], "foo"],
  [["aaa\\bbb", "\\bbb"], "bbb"],
  [["aaa\\bbb", "a\\bbb"], "bbb"],
  [["aaa\\bbb", "bbb"], "bbb"],
  [["aaa\\bbb\\\\\\\\", "bbb"], "bbb"],
  [["aaa\\bbb", "bb"], "b"],
  [["aaa\\bbb", "b"], "bb"],
  [["/aaa/bbb", "bb"], "b"],
  [["C:"], ""],
  [["C:."], "."],
  [["C:\\"], "\\"],
  [["C:\\dir\\base.ext"], "base.ext"],
  [["C:\\basename.ext"], "basename.ext"],
  [["C:basename.ext"], "basename.ext"],
  [["C:basename.ext\\"], "basename.ext"],
  [["C:basename.ext\\\\"], "basename.ext"],
  [["C:foo"], "foo"],
  [["file:stream"], "file:stream"],
];

Deno.test("posix.basename()", function () {
  for (const [[name, suffix], expected] of COREUTILS_TESTSUITE) {
    assertEquals(path.basename(name, suffix), expected);
  }

  for (const [[name, suffix], expected] of POSIX_TESTSUITE) {
    assertEquals(path.posix.basename(name, suffix), expected);
  }

  // On unix a backslash is just treated as any other character.
  assertEquals(
    path.posix.basename("\\dir\\basename.ext"),
    "\\dir\\basename.ext",
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
    controlCharFilename,
  );
});

Deno.test("win32.basename()", function () {
  for (const [[name, suffix], expected] of WIN32_TESTSUITE) {
    assertEquals(path.win32.basename(name, suffix), expected);
  }

  // path.win32 should pass all "forward slash" posix tests as well.
  for (const [[name, suffix], expected] of COREUTILS_TESTSUITE) {
    assertEquals(path.win32.basename(name, suffix), expected);
  }

  for (const [[name, suffix], expected] of POSIX_TESTSUITE) {
    assertEquals(path.win32.basename(name, suffix), expected);
  }
});
