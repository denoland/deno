// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

import { test, assertEqual } from "../../testing/mod.ts";
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
    assertEqual(expected, path.posix.extname(input));
  });

  // On *nix, backslash is a valid name component like any other character.
  assertEqual(path.posix.extname(".\\"), "");
  assertEqual(path.posix.extname("..\\"), ".\\");
  assertEqual(path.posix.extname("file.ext\\"), ".ext\\");
  assertEqual(path.posix.extname("file.ext\\\\"), ".ext\\\\");
  assertEqual(path.posix.extname("file\\"), "");
  assertEqual(path.posix.extname("file\\\\"), "");
  assertEqual(path.posix.extname("file.\\"), ".\\");
  assertEqual(path.posix.extname("file.\\\\"), ".\\\\");
});

test(function extnameWin32() {
  pairs.forEach(function(p) {
    const input = p[0].replace(slashRE, "\\");
    const expected = p[1];
    assertEqual(expected, path.win32.extname(input));
    assertEqual(expected, path.win32.extname("C:" + input));
  });

  // On Windows, backslash is a path separator.
  assertEqual(path.win32.extname(".\\"), "");
  assertEqual(path.win32.extname("..\\"), "");
  assertEqual(path.win32.extname("file.ext\\"), ".ext");
  assertEqual(path.win32.extname("file.ext\\\\"), ".ext");
  assertEqual(path.win32.extname("file\\"), "");
  assertEqual(path.win32.extname("file\\\\"), "");
  assertEqual(path.win32.extname("file.\\"), ".");
  assertEqual(path.win32.extname("file.\\\\"), ".");
});
