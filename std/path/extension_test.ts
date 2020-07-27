// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals } from "../testing/asserts.ts";
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
  ["file.//", "."],
];

Deno.test("extension", function () {
  pairs.forEach(function (p) {
    const input = p[0];
    const expected = p[1];
    assertEquals(expected, path.posix.extension(input));
  });

  // On *nix, backslash is a valid name component like any other character.
  assertEquals(path.posix.extension(".\\"), "");
  assertEquals(path.posix.extension("..\\"), ".\\");
  assertEquals(path.posix.extension("file.ext\\"), ".ext\\");
  assertEquals(path.posix.extension("file.ext\\\\"), ".ext\\\\");
  assertEquals(path.posix.extension("file\\"), "");
  assertEquals(path.posix.extension("file\\\\"), "");
  assertEquals(path.posix.extension("file.\\"), ".\\");
  assertEquals(path.posix.extension("file.\\\\"), ".\\\\");
});

Deno.test("extensionWin32", function () {
  pairs.forEach(function (p) {
    const input = p[0].replace(slashRE, "\\");
    const expected = p[1];
    assertEquals(expected, path.win32.extension(input));
    assertEquals(expected, path.win32.extension("C:" + input));
  });

  // On Windows, backslash is a path separator.
  assertEquals(path.win32.extension(".\\"), "");
  assertEquals(path.win32.extension("..\\"), "");
  assertEquals(path.win32.extension("file.ext\\"), ".ext");
  assertEquals(path.win32.extension("file.ext\\\\"), ".ext");
  assertEquals(path.win32.extension("file\\"), "");
  assertEquals(path.win32.extension("file\\\\"), "");
  assertEquals(path.win32.extension("file.\\"), ".");
  assertEquals(path.win32.extension("file.\\\\"), ".");
});
