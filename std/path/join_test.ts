// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import * as path from "./mod.ts";

const backslashRE = /\\/g;

const joinTests =
  // arguments                     result
  [
    [[".", "x/b", "..", "/b/c.js"], "x/b/c.js"],
    [[], "."],
    [["/.", "x/b", "..", "/b/c.js"], "/x/b/c.js"],
    [["/foo", "../../../bar"], "/bar"],
    [["foo", "../../../bar"], "../../bar"],
    [["foo/", "../../../bar"], "../../bar"],
    [["foo/x", "../../../bar"], "../bar"],
    [["foo/x", "./bar"], "foo/x/bar"],
    [["foo/x/", "./bar"], "foo/x/bar"],
    [["foo/x/", ".", "bar"], "foo/x/bar"],
    [["./"], "./"],
    [[".", "./"], "./"],
    [[".", ".", "."], "."],
    [[".", "./", "."], "."],
    [[".", "/./", "."], "."],
    [[".", "/////./", "."], "."],
    [["."], "."],
    [["", "."], "."],
    [["", "foo"], "foo"],
    [["foo", "/bar"], "foo/bar"],
    [["", "/foo"], "/foo"],
    [["", "", "/foo"], "/foo"],
    [["", "", "foo"], "foo"],
    [["foo", ""], "foo"],
    [["foo/", ""], "foo/"],
    [["foo", "", "/bar"], "foo/bar"],
    [["./", "..", "/foo"], "../foo"],
    [["./", "..", "..", "/foo"], "../../foo"],
    [[".", "..", "..", "/foo"], "../../foo"],
    [["", "..", "..", "/foo"], "../../foo"],
    [["/"], "/"],
    [["/", "."], "/"],
    [["/", ".."], "/"],
    [["/", "..", ".."], "/"],
    [[""], "."],
    [["", ""], "."],
    [[" /foo"], " /foo"],
    [[" ", "foo"], " /foo"],
    [[" ", "."], " "],
    [[" ", "/"], " /"],
    [[" ", ""], " "],
    [["/", "foo"], "/foo"],
    [["/", "/foo"], "/foo"],
    [["/", "//foo"], "/foo"],
    [["/", "", "/foo"], "/foo"],
    [["", "/", "foo"], "/foo"],
    [["", "/", "/foo"], "/foo"],
  ];

// Windows-specific join tests
const windowsJoinTests = [
  // arguments                     result
  // UNC path expected
  [["//foo/bar"], "\\\\foo\\bar\\"],
  [["\\/foo/bar"], "\\\\foo\\bar\\"],
  [["\\\\foo/bar"], "\\\\foo\\bar\\"],
  // UNC path expected - server and share separate
  [["//foo", "bar"], "\\\\foo\\bar\\"],
  [["//foo/", "bar"], "\\\\foo\\bar\\"],
  [["//foo", "/bar"], "\\\\foo\\bar\\"],
  // UNC path expected - questionable
  [["//foo", "", "bar"], "\\\\foo\\bar\\"],
  [["//foo/", "", "bar"], "\\\\foo\\bar\\"],
  [["//foo/", "", "/bar"], "\\\\foo\\bar\\"],
  // UNC path expected - even more questionable
  [["", "//foo", "bar"], "\\\\foo\\bar\\"],
  [["", "//foo/", "bar"], "\\\\foo\\bar\\"],
  [["", "//foo/", "/bar"], "\\\\foo\\bar\\"],
  // No UNC path expected (no double slash in first component)
  [["\\", "foo/bar"], "\\foo\\bar"],
  [["\\", "/foo/bar"], "\\foo\\bar"],
  [["", "/", "/foo/bar"], "\\foo\\bar"],
  // No UNC path expected (no non-slashes in first component -
  // questionable)
  [["//", "foo/bar"], "\\foo\\bar"],
  [["//", "/foo/bar"], "\\foo\\bar"],
  [["\\\\", "/", "/foo/bar"], "\\foo\\bar"],
  [["//"], "\\"],
  // No UNC path expected (share name missing - questionable).
  [["//foo"], "\\foo"],
  [["//foo/"], "\\foo\\"],
  [["//foo", "/"], "\\foo\\"],
  [["//foo", "", "/"], "\\foo\\"],
  // No UNC path expected (too many leading slashes - questionable)
  [["///foo/bar"], "\\foo\\bar"],
  [["////foo", "bar"], "\\foo\\bar"],
  [["\\\\\\/foo/bar"], "\\foo\\bar"],
  // Drive-relative vs drive-absolute paths. This merely describes the
  // status quo, rather than being obviously right
  [["c:"], "c:."],
  [["c:."], "c:."],
  [["c:", ""], "c:."],
  [["", "c:"], "c:."],
  [["c:.", "/"], "c:.\\"],
  [["c:.", "file"], "c:file"],
  [["c:", "/"], "c:\\"],
  [["c:", "file"], "c:\\file"],
];

Deno.test("join", function () {
  joinTests.forEach(function (p) {
    const _p = p[0] as string[];
    const actual = path.posix.join.apply(null, _p);
    assertEquals(actual, p[1]);
  });
});

Deno.test("joinWin32", function () {
  joinTests.forEach(function (p) {
    const _p = p[0] as string[];
    const actual = path.win32.join.apply(null, _p).replace(backslashRE, "/");
    assertEquals(actual, p[1]);
  });
  windowsJoinTests.forEach(function (p) {
    const _p = p[0] as string[];
    const actual = path.win32.join.apply(null, _p);
    assertEquals(actual, p[1]);
  });
});
