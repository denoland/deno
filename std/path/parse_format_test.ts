// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import type { FormatInputPathObject, ParsedPath } from "./mod.ts";

import { assertEquals } from "../testing/asserts.ts";
import { posix, win32 } from "./mod.ts";

type FormatTestCase = [FormatInputPathObject, string];
type ParseTestCase = [string, ParsedPath];

const winPaths: Array<[string, string]> = [
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

const winSpecialCaseParseTests: ParseTestCase[] = [
  ["/foo/bar", { root: "/", dir: "/foo", base: "bar", ext: "", name: "bar" }],
];

const winSpecialCaseFormatTests: FormatTestCase[] = [
  [{ dir: "some\\dir" }, "some\\dir\\"],
  [{ base: "index.html" }, "index.html"],
  [{ root: "C:\\" }, "C:\\"],
  [{ name: "index", ext: ".html" }, "index.html"],
  [{ dir: "some\\dir", name: "index", ext: ".html" }, "some\\dir\\index.html"],
  [{ root: "C:\\", name: "index", ext: ".html" }, "C:\\index.html"],
  [{}, ""],
];

const unixPaths: Array<[string, string]> = [
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

const unixSpecialCaseFormatTests: FormatTestCase[] = [
  [{ dir: "some/dir" }, "some/dir/"],
  [{ base: "index.html" }, "index.html"],
  [{ root: "/" }, "/"],
  [{ name: "index", ext: ".html" }, "index.html"],
  [{ dir: "some/dir", name: "index", ext: ".html" }, "some/dir/index.html"],
  [{ root: "/", name: "index", ext: ".html" }, "/index.html"],
  [{}, ""],
];

function checkParseFormat(
  path: typeof win32 | typeof posix,
  testCases: Array<[string, string]>,
): void {
  testCases.forEach(([element, root]) => {
    const output = path.parse(element);
    assertEquals(typeof output.root, "string");
    assertEquals(typeof output.dir, "string");
    assertEquals(typeof output.base, "string");
    assertEquals(typeof output.ext, "string");
    assertEquals(typeof output.name, "string");
    assertEquals(path.format(output), element);
    assertEquals(output.root, root);
    assertEquals(output.dir, output.dir ? path.dirname(element) : "");
    assertEquals(output.base, path.fileName(element) ?? "");
    assertEquals(output.ext, path.extname(element));
  });
}

function checkSpecialCaseParseFormat(
  path: typeof win32 | typeof posix,
  testCases: ParseTestCase[],
): void {
  testCases.forEach(([element, expect]) => {
    assertEquals(path.parse(element), expect);
  });
}

function checkFormat(
  path: typeof win32 | typeof posix,
  testCases: FormatTestCase[],
): void {
  testCases.forEach((testCase) => {
    assertEquals(path.format(testCase[0]), testCase[1]);
  });
}

Deno.test("parseWin32", function () {
  checkParseFormat(win32, winPaths);
  checkSpecialCaseParseFormat(win32, winSpecialCaseParseTests);
});

Deno.test("parse", function () {
  checkParseFormat(posix, unixPaths);
});

Deno.test("formatWin32", function () {
  checkFormat(win32, winSpecialCaseFormatTests);
});

Deno.test("format", function () {
  checkFormat(posix, unixSpecialCaseFormatTests);
});

// Test removal of trailing path separators
const windowsTrailingTests: ParseTestCase[] = [
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

const posixTrailingTests: ParseTestCase[] = [
  ["./", { root: "", dir: "", base: ".", ext: "", name: "." }],
  ["//", { root: "/", dir: "/", base: "", ext: "", name: "" }],
  ["///", { root: "/", dir: "/", base: "", ext: "", name: "" }],
  ["/foo///", { root: "/", dir: "/", base: "foo", ext: "", name: "foo" }],
  [
    "/foo///bar.baz",
    { root: "/", dir: "/foo//", base: "bar.baz", ext: ".baz", name: "bar" },
  ],
];

Deno.test("parseTrailingWin32", function () {
  windowsTrailingTests.forEach(function (p) {
    const actual = win32.parse(p[0]);
    const expected = p[1];
    assertEquals(actual, expected);
  });
});

Deno.test("parseTrailing", function () {
  posixTrailingTests.forEach(function (p) {
    const actual = posix.parse(p[0]);
    const expected = p[1];
    assertEquals(actual, expected);
  });
});
