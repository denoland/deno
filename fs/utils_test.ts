// Copyright the Browserify authors. MIT License.

import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { isSubdir } from "./utils.ts";
import * as path from "./path/mod.ts";

test(function _isSubdir() {
  const pairs = [
    ["", "", false, path.posix.sep],
    ["/first/second", "/first", false, path.posix.sep],
    ["/first", "/first", false, path.posix.sep],
    ["/first", "/first/second", true, path.posix.sep],
    ["first", "first/second", true, path.posix.sep],
    ["../first", "../first/second", true, path.posix.sep],
    ["c:\\first", "c:\\first", false, path.win32.sep],
    ["c:\\first", "c:\\first\\second", true, path.win32.sep]
  ];

  pairs.forEach(function(p) {
    const src = p[0] as string;
    const dest = p[1] as string;
    const expected = p[2] as boolean;
    const sep = p[3] as string;
    assertEquals(
      isSubdir(src, dest, sep),
      expected,
      `'${src}' should ${expected ? "" : "not"} be parent dir of '${dest}'`
    );
  });
});
