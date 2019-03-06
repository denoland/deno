// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import * as path from "./mod.ts";

const slashRE = /\//g;

const pairs = [
  ["", ""],
  ["/path/to/file", ""],
  ["/path/to/file.ext", ".ext"],
  ["/path.to/file.ext", ".ext"],
  ["/path.to/file", ""],
  ["/path.to/.file", ""],
  ["/path.to/.file.ext", ".ext"],
  ["/path/to/f.ext", ".ext"],
  ["/path/to/..ext", ".ext"],
  ["/path/to/..", ""],
  ["file", ""],
  ["file.ext", ".ext"],
  [".file", ""],
  [".file.ext", ".ext"],
  ["/file", ""],
  ["/file.ext", ".ext"],
  ["/.file", ""],
  ["/.file.ext", ".ext"],
  [".path/file.ext", ".ext"],
  ["file.ext.ext", ".ext"],
  ["file.", "."],
  [".", ""],
  ["./", ""],
  [".file.ext", ".ext"],
  [".file", ""],
  [".file.", "."],
  [".file..", "."],
  ["..", ""],
  ["../", ""],
  ["..file.ext", ".ext"],
  ["..file", ".file"],
  ["..file.", "."],
  ["..file..", "."],
  ["...", "."],
  ["...ext", ".ext"],
  ["....", "."],
  ["file.ext/", ".ext"],
  ["file.ext//", ".ext"],
  ["file/", ""],
  ["file//", ""],
  ["file./", "."],
  ["file.//", "."]
];

test(function extname() {
  pairs.forEach(function(p) {
    const input = p[0];
    const expected = p[1];
    assertEq(expected, path.posix.extname(input));
  });

  // On *nix, backslash is a valid name component like any other character.
  assertEq(path.posix.extname(".\\"), "");
  assertEq(path.posix.extname("..\\"), ".\\");
  assertEq(path.posix.extname("file.ext\\"), ".ext\\");
  assertEq(path.posix.extname("file.ext\\\\"), ".ext\\\\");
  assertEq(path.posix.extname("file\\"), "");
  assertEq(path.posix.extname("file\\\\"), "");
  assertEq(path.posix.extname("file.\\"), ".\\");
  assertEq(path.posix.extname("file.\\\\"), ".\\\\");
});

test(function extnameWin32() {
  pairs.forEach(function(p) {
    const input = p[0].replace(slashRE, "\\");
    const expected = p[1];
    assertEq(expected, path.win32.extname(input));
    assertEq(expected, path.win32.extname("C:" + input));
  });

  // On Windows, backslash is a path separator.
  assertEq(path.win32.extname(".\\"), "");
  assertEq(path.win32.extname("..\\"), "");
  assertEq(path.win32.extname("file.ext\\"), ".ext");
  assertEq(path.win32.extname("file.ext\\\\"), ".ext");
  assertEq(path.win32.extname("file\\"), "");
  assertEq(path.win32.extname("file\\\\"), "");
  assertEq(path.win32.extname("file.\\"), ".");
  assertEq(path.win32.extname("file.\\\\"), ".");
});
