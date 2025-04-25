// Copyright 2018-2025 the Deno authors. MIT license.
import sqlite, { DatabaseSync } from "node:sqlite";
import { assert, assertEquals, assertThrows } from "@std/assert";
import * as path from "node:path";

// Choose the correct extension based on the platform
const extensionPath = (() => {
  const isWindows = Deno.build.os === "windows";
  const isMac = Deno.build.os === "darwin";
  const isLinux = Deno.build.os === "linux";

  // Get the path to the target directory
  const currentDir = new URL(".", import.meta.url).pathname;
  const denoDir = path.resolve(currentDir, "..");

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

Deno.test({
  name: "[node/sqlite] DatabaseSync loadExtension with allowExtension enabled",
  name: "[node/sqlite] DatabaseSync loadExtension option handling",
  permissions: { read: true, write: true, ffi: true },
  fn() {
    // Check if the extension file exists - if not, skip the test
    try {
      Deno.statSync(extensionPath);
    } catch (e) {
      console.log(`Extension not found at ${extensionPath}, skipping test`);
      return;
    }

    // DatabaseSync with allowExtension: true should work
    const db = new DatabaseSync(":memory:", {
      allowExtension: true,
      readOnly: false,
    });

    // Load the extension
    db.loadExtension(extensionPath);

    // Test that the function is accessible
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
    // Even creating a DB with allowExtension: true should throw when FFI permission is denied
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
