// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
/* eslint-disable @typescript-eslint/no-explicit-any */
// TODO(kt3k): fix any types in this file

const { test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import * as path from "./mod.ts";

const winPaths = [
  // [path, root]
  ["C:\\path\\dir\\index.html", "C:\\"],
  ["C:\\another_path\\DIR\\1\\2\\33\\\\index", "C:\\"],
  ["another_path\\DIR with spaces\\1\\2\\33\\index", ""],
  ["\\", "\\"],
  ["\\foo\\C:", "\\"],
  ["file", ""],
  ["file:stream", ""],
  [".\\file", ""],
  ["C:", "C:"],
  ["C:.", "C:"],
  ["C:..", "C:"],
  ["C:abc", "C:"],
  ["C:\\", "C:\\"],
  ["C:\\abc", "C:\\"],
  ["", ""],
  // unc
  ["\\\\server\\share\\file_path", "\\\\server\\share\\"],
  [
    "\\\\server two\\shared folder\\file path.zip",
    "\\\\server two\\shared folder\\",
  ],
  ["\\\\teela\\admin$\\system32", "\\\\teela\\admin$\\"],
  ["\\\\?\\UNC\\server\\share", "\\\\?\\UNC\\"],
];

const winSpecialCaseParseTests = [["/foo/bar", { root: "/" }]];

const winSpecialCaseFormatTests = [
  [{ dir: "some\\dir" }, "some\\dir\\"],
  [{ base: "index.html" }, "index.html"],
  [{ root: "C:\\" }, "C:\\"],
  [{ name: "index", ext: ".html" }, "index.html"],
  [{ dir: "some\\dir", name: "index", ext: ".html" }, "some\\dir\\index.html"],
  [{ root: "C:\\", name: "index", ext: ".html" }, "C:\\index.html"],
  [{}, ""],
];

const unixPaths = [
  // [path, root]
  ["/home/user/dir/file.txt", "/"],
  ["/home/user/a dir/another File.zip", "/"],
  ["/home/user/a dir//another&File.", "/"],
  ["/home/user/a$$$dir//another File.zip", "/"],
  ["user/dir/another File.zip", ""],
  ["file", ""],
  [".\\file", ""],
  ["./file", ""],
  ["C:\\foo", ""],
  ["/", "/"],
  ["", ""],
  [".", ""],
  ["..", ""],
  ["/foo", "/"],
  ["/foo.", "/"],
  ["/foo.bar", "/"],
  ["/.", "/"],
  ["/.foo", "/"],
  ["/.foo.bar", "/"],
  ["/foo/bar.baz", "/"],
];

const unixSpecialCaseFormatTests = [
  [{ dir: "some/dir" }, "some/dir/"],
  [{ base: "index.html" }, "index.html"],
  [{ root: "/" }, "/"],
  [{ name: "index", ext: ".html" }, "index.html"],
  [{ dir: "some/dir", name: "index", ext: ".html" }, "some/dir/index.html"],
  [{ root: "/", name: "index", ext: ".html" }, "/index.html"],
  [{}, ""],
];

function checkParseFormat(path: any, paths: any): void {
  paths.forEach(function (p: Array<Record<string, unknown>>) {
    const element = p[0];
    const output = path.parse(element);
    assertEquals(typeof output.root, "string");
    assertEquals(typeof output.dir, "string");
    assertEquals(typeof output.base, "string");
    assertEquals(typeof output.ext, "string");
    assertEquals(typeof output.name, "string");
    assertEquals(path.format(output), element);
    assertEquals(output.rooroot, undefined);
    assertEquals(output.dir, output.dir ? path.dirname(element) : "");
    assertEquals(output.base, path.basename(element));
  });
}

function checkSpecialCaseParseFormat(path: any, testCases: any): void {
  testCases.forEach(function (testCase: Array<Record<string, unknown>>) {
    const element = testCase[0];
    const expect = testCase[1];
    const output = path.parse(element);
    Object.keys(expect).forEach(function (key) {
      assertEquals(output[key], expect[key]);
    });
  });
}

function checkFormat(path: any, testCases: unknown[][]): void {
  testCases.forEach(function (testCase) {
    assertEquals(path.format(testCase[0]), testCase[1]);
  });
}

test(function parseWin32() {
  checkParseFormat(path.win32, winPaths);
  checkSpecialCaseParseFormat(path.win32, winSpecialCaseParseTests);
});

test(function parse() {
  checkParseFormat(path.posix, unixPaths);
});

test(function formatWin32() {
  checkFormat(path.win32, winSpecialCaseFormatTests);
});

test(function format() {
  checkFormat(path.posix, unixSpecialCaseFormatTests);
});

// Test removal of trailing path separators
const windowsTrailingTests = [
  [".\\", { root: "", dir: "", base: ".", ext: "", name: "." }],
  ["\\\\", { root: "\\", dir: "\\", base: "", ext: "", name: "" }],
  ["\\\\", { root: "\\", dir: "\\", base: "", ext: "", name: "" }],
  [
    "c:\\foo\\\\\\",
    { root: "c:\\", dir: "c:\\", base: "foo", ext: "", name: "foo" },
  ],
  [
    "D:\\foo\\\\\\bar.baz",
    {
      root: "D:\\",
      dir: "D:\\foo\\\\",
      base: "bar.baz",
      ext: ".baz",
      name: "bar",
    },
  ],
];

const posixTrailingTests = [
  ["./", { root: "", dir: "", base: ".", ext: "", name: "." }],
  ["//", { root: "/", dir: "/", base: "", ext: "", name: "" }],
  ["///", { root: "/", dir: "/", base: "", ext: "", name: "" }],
  ["/foo///", { root: "/", dir: "/", base: "foo", ext: "", name: "foo" }],
  [
    "/foo///bar.baz",
    { root: "/", dir: "/foo//", base: "bar.baz", ext: ".baz", name: "bar" },
  ],
];

test(function parseTrailingWin32() {
  windowsTrailingTests.forEach(function (p) {
    const actual = path.win32.parse(p[0] as string);
    const expected = p[1];
    assertEquals(actual, expected);
  });
});

test(function parseTrailing() {
  posixTrailingTests.forEach(function (p) {
    const actual = path.posix.parse(p[0] as string);
    const expected = p[1];
    assertEquals(actual, expected);
  });
});
