// Copyright the Browserify authors. MIT License.

import { assertEquals } from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import { getFileInfoType, isSubdir, PathType } from "./_util.ts";
import { ensureFileSync } from "./ensure_file.ts";
import { ensureDirSync } from "./ensure_dir.ts";

const moduleDir = path.parent(path.fromFileUrl(import.meta.url));
const testdataDir = path.resolve(moduleDir, "testdata");

Deno.test("_isSubdir", function (): void {
  const pairs = [
    ["", "", false, path.posix.separator],
    ["/first/second", "/first", false, path.posix.separator],
    ["/first", "/first", false, path.posix.separator],
    ["/first", "/first/second", true, path.posix.separator],
    ["first", "first/second", true, path.posix.separator],
    ["../first", "../first/second", true, path.posix.separator],
    ["c:\\first", "c:\\first", false, path.win32.separator],
    ["c:\\first", "c:\\first\\second", true, path.win32.separator],
  ];

  pairs.forEach(function (p): void {
    const src = p[0] as string;
    const dest = p[1] as string;
    const expected = p[2] as boolean;
    const sep = p[3] as string;
    assertEquals(
      isSubdir(src, dest, sep),
      expected,
      `'${src}' should ${expected ? "" : "not"} be parent dir of '${dest}'`,
    );
  });
});

Deno.test("_getFileInfoType", function (): void {
  const pairs = [
    [path.join(testdataDir, "file_type_1"), "file"],
    [path.join(testdataDir, "file_type_dir_1"), "dir"],
  ];

  pairs.forEach(function (p): void {
    const filePath = p[0] as string;
    const type = p[1] as PathType;
    switch (type) {
      case "file":
        ensureFileSync(filePath);
        break;
      case "dir":
        ensureDirSync(filePath);
        break;
      case "symlink":
        // TODO(axetroy): test symlink
        break;
    }

    const stat = Deno.statSync(filePath);

    Deno.removeSync(filePath, { recursive: true });

    assertEquals(getFileInfoType(stat), type);
  });
});
