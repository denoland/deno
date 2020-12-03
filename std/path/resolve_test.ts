// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals, assertThrows } from "../testing/asserts.ts";
import * as path from "./mod.ts";

const windowsPassTests =
  // arguments                               result
  [
    [["c:/blah\\blah", "d:/games", "c:../a"], "c:\\blah\\a"],
    [["c:/ignore", "d:\\a/b\\c/d", "\\e.exe"], "d:\\e.exe"],
    [["c:/ignore", "c:/some/file"], "c:\\some\\file"],
    [["d:/ignore", "d:some/dir//"], "d:\\ignore\\some\\dir"],
    [["//server/share", "..", "relative\\"], "\\\\server\\share\\relative"],
    [["c:/", "//"], "c:\\"],
    [["c:/", "//dir"], "c:\\dir"],
    [["c:/", "//server/share"], "\\\\server\\share\\"],
    [["c:/", "//server//share"], "\\\\server\\share\\"],
    [["c:/", "///some//dir"], "c:\\some\\dir"],
    [
      ["C:\\foo\\tmp.3\\", "..\\tmp.3\\cycles\\root.js"],
      "C:\\foo\\tmp.3\\cycles\\root.js",
    ],
  ];

const posixPassTests =
  // arguments                    result
  [
    [["/var/lib", "../", "file/"], "/var/file"],
    [["/var/lib", "/../", "file/"], "/file"],
    [["/some/dir", ".", "/absolute/"], "/absolute"],
    [["/foo/tmp.3/", "../tmp.3/cycles/root.js"], "/foo/tmp.3/cycles/root.js"],
    [["/"], "/"],
  ];

const posixThrowTests =
  // arguments                    result
  [
    ["a/b/c/", "../../.."],
    ["."],
  ];

const windowsThrowTests =
  // arguments                    result
  [
    ["a\\b\\c\\", "..\\..\\.."],
    ["."],
  ];

Deno.test("resolvePasses", function () {
  posixPassTests.forEach(function (p) {
    const _p = p[0] as string[];
    const actual = path.posix.resolve.apply(null, _p);
    assertEquals(actual, p[1]);
  });
});

Deno.test("resolveThrows", function () {
  posixThrowTests.forEach(function (p) {
    const _p = p as string[];
    assertThrows(() => {
      path.posix.resolve.apply(null, _p);
    });
  });
});

Deno.test("resolveWin32Passes", function () {
  windowsPassTests.forEach(function (p) {
    const _p = p[0] as string[];
    const actual = path.win32.resolve.apply(null, _p);
    assertEquals(actual, p[1]);
  });
});

Deno.test("resolveWin32Throws", function () {
  windowsThrowTests.forEach(function (p) {
    const _p = p as string[];
    assertThrows(() => {
      path.win32.resolve.apply(null, _p);
    });
  });
});
