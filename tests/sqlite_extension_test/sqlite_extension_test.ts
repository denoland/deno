// Copyright 2018-2025 the Deno authors. MIT license.

// NOTE: This test requires building the extension separately (it is excluded
// from the workspace due to incompatible rusqlite feature requirements):
//   cargo build --manifest-path tests/sqlite_extension/Cargo.toml

import { DatabaseSync } from "node:sqlite";
import { assertEquals, assertThrows } from "@std/assert";
import * as path from "node:path";

const extensionPath = (() => {
  const isWindows = Deno.build.os === "windows";
  const isMac = Deno.build.os === "darwin";
  const isLinux = Deno.build.os === "linux";

  const currentDir = new URL(".", import.meta.url).pathname;
  const denoDir = path.resolve(currentDir, "../..");

  let libPrefix = "";
  let libSuffix = "";

  if (isWindows) {
    libSuffix = "dll";
  } else if (isMac) {
    libPrefix = "lib";
    libSuffix = "dylib";
  } else if (isLinux) {
    libPrefix = "lib";
    libSuffix = "so";
  } else {
    throw new Error("Unsupported platform");
  }

  const targetDir = path.join(denoDir, "target", "debug");
  return path.join(targetDir, `${libPrefix}test_sqlite_extension.${libSuffix}`);
})();

const extensionExists = (() => {
  try {
    Deno.statSync(extensionPath);
    return true;
  } catch {
    return false;
  }
})();

Deno.test({
  name: "[node/sqlite] DatabaseSync loadExtension",
  ignore: !extensionExists,
  permissions: { read: true, write: true, ffi: true },
  fn() {
    const db = new DatabaseSync(":memory:", {
      allowExtension: true,
      readOnly: false,
    });

    db.loadExtension(extensionPath);

    const stmt = db.prepare("SELECT test_func('Hello, World!') AS result");
    const { result } = stmt.get();
    assertEquals(result, "Hello, World!");

    db.close();
  },
});

Deno.test({
  name: "[node/sqlite] DatabaseSync loadExtension with FFI permission denied",
  permissions: { read: true, write: true, ffi: false },
  fn() {
    assertThrows(() => {
      new DatabaseSync(":memory:", {
        allowExtension: true,
        readOnly: false,
      });
    }, Deno.errors.NotCapable);
  },
});

Deno.test({
  name: "[node/sqlite] DatabaseSync loadExtension with invalid path",
  permissions: { read: true, write: true, ffi: true },
  fn() {
    const db = new DatabaseSync(":memory:", {
      allowExtension: true,
      readOnly: false,
    });

    // Loading a non-existent extension should throw
    assertThrows(() => {
      db.loadExtension("/path/to/nonexistent/extension");
    }, Error);

    db.close();
  },
});
