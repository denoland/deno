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
    const { result } = stmt.get()!;
    assertEquals(result, "Hello, World!");

    db.close();
  },
});

Deno.test({
  name: "[node/sqlite] DatabaseSync loadExtension with FFI permission denied",
  permissions: { read: true, write: true, ffi: false },
  fn() {
    // Creating a database with allowExtension: true should succeed
    // (permission check deferred to loadExtension)
    const db = new DatabaseSync(":memory:", {
      allowExtension: true,
      readOnly: false,
    });

    // The error should occur when actually trying to load an extension
    assertThrows(() => {
      db.loadExtension("/some/extension/path");
    }, Deno.errors.NotCapable);

    db.close();
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

// Tests for scoped FFI permissions (--allow-ffi=/path/to/extension)
// These require subprocess spawning since Deno.test permissions don't support scoped FFI

Deno.test({
  name: "[node/sqlite] DatabaseSync with scoped FFI permission succeeds",
  ignore: !extensionExists,
  permissions: { read: true, run: true },
  async fn() {
    const code = `
      import { DatabaseSync } from "node:sqlite";
      const extensionPath = Deno.args[0];
      const db = new DatabaseSync(":memory:", { allowExtension: true });
      db.loadExtension(extensionPath);
      const stmt = db.prepare("SELECT test_func('test') AS result");
      const { result } = stmt.get()!;
      if (result !== "test") throw new Error("Unexpected result: " + result);
      db.close();
      console.log("OK");
    `;

    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        `--allow-read=${extensionPath}`,
        `--allow-ffi=${extensionPath}`,
        "--no-lock",
        "-",
        extensionPath,
      ],
      stdin: "piped",
      stdout: "piped",
      stderr: "piped",
    });

    const child = command.spawn();
    const writer = child.stdin.getWriter();
    await writer.write(new TextEncoder().encode(code));
    await writer.close();

    const { code: exitCode, stdout, stderr } = await child.output();
    const stdoutText = new TextDecoder().decode(stdout);
    const stderrText = new TextDecoder().decode(stderr);

    assertEquals(exitCode, 0, `Expected success but got: ${stderrText}`);
    assertEquals(stdoutText.trim(), "OK");
  },
});

Deno.test({
  name:
    "[node/sqlite] DatabaseSync loadExtension fails for path outside scoped FFI",
  ignore: !extensionExists,
  permissions: { read: true, run: true },
  async fn() {
    // Grant FFI only for a different path, not the actual extension
    const wrongPath = "/some/other/path";

    const code = `
      import { DatabaseSync } from "node:sqlite";
      const extensionPath = Deno.args[0];
      const db = new DatabaseSync(":memory:", { allowExtension: true });
      try {
        db.loadExtension(extensionPath);
        console.log("UNEXPECTED_SUCCESS");
      } catch (e) {
        if (e instanceof Deno.errors.NotCapable) {
          console.log("EXPECTED_PERMISSION_ERROR");
        } else {
          console.log("UNEXPECTED_ERROR: " + e.constructor.name + ": " + e.message);
        }
      }
      db.close();
    `;

    const command = new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        `--allow-read=${extensionPath}`,
        `--allow-ffi=${wrongPath}`,
        "--no-lock",
        "-",
        extensionPath,
      ],
      stdin: "piped",
      stdout: "piped",
      stderr: "piped",
    });

    const child = command.spawn();
    const writer = child.stdin.getWriter();
    await writer.write(new TextEncoder().encode(code));
    await writer.close();

    const { stdout } = await child.output();
    const stdoutText = new TextDecoder().decode(stdout);

    assertEquals(
      stdoutText.trim(),
      "EXPECTED_PERMISSION_ERROR",
      `Expected NotCapable error but got: ${stdoutText}`,
    );
  },
});

Deno.test({
  name:
    "[node/sqlite] SQL load_extension() is disabled even with allowExtension: true",
  ignore: !extensionExists,
  permissions: { read: true, write: true, ffi: true },
  fn() {
    // Even with allowExtension: true and full FFI permissions,
    // the SQL function load_extension() should be disabled.
    // Only the C API loadExtension() method should work.
    const db = new DatabaseSync(":memory:", {
      allowExtension: true,
      readOnly: false,
    });

    // Attempting to load extension via SQL should fail with "not authorized",
    // even though the same extension loads successfully via C API
    const loadExtStmt = db.prepare("SELECT load_extension($path)");
    assertThrows(
      () => {
        loadExtStmt.get({ $path: extensionPath });
      },
      Error,
      "not authorized",
    );

    // Verify the C API still works with the same extension
    db.loadExtension(extensionPath);
    const stmt = db.prepare("SELECT test_func('works') AS result");
    const { result } = stmt.get()!;
    assertEquals(result, "works");

    db.close();
  },
});
