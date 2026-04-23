// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any

import { assert } from "@std/assert";
import { isatty } from "node:tty";
import tty from "node:tty";
import process from "node:process";
import fs from "node:fs";

Deno.test("[node/tty isatty] returns true when fd is a tty, false otherwise", () => {
  // Uses raw file descriptors: 0 = stdin, 1 = stdout, 2 = stderr
  assert(Deno.stdin.isTerminal() === isatty(0));
  assert(Deno.stdout.isTerminal() === isatty(1));
  assert(Deno.stderr.isTerminal() === isatty(2));
});

Deno.test("[node/tty isatty] returns false for irrelevant values", () => {
  // invalid numeric fd
  assert(!isatty(1234567));

  // negative fd should return false
  assert(!isatty(-1));

  // non-integer numeric fd should return false
  assert(!isatty(0.5));
  assert(!isatty(1.3));
  assert(!isatty(2.2));
  assert(!isatty(3.1));

  // invalid type fd
  assert(!isatty("abc" as any));
  assert(!isatty({} as any));
  assert(!isatty([] as any));
  assert(!isatty(null as any));
  assert(!isatty(undefined as any));
});

Deno.test("[node/tty WriteStream.isTTY] returns true when fd is a tty", () => {
  assert(Deno.stdin.isTerminal() === process.stdin.isTTY);
  assert(Deno.stdout.isTerminal() === process.stdout.isTTY);
});

Deno.test("[node/tty WriteStream.hasColors] returns true when colors are supported", () => {
  const stubEnv = Deno.noColor ? { NO_COLOR: "1" } : {};

  assert(tty.WriteStream.prototype.hasColors() === !Deno.noColor);
  assert(tty.WriteStream.prototype.hasColors(stubEnv) === !Deno.noColor);

  assert(tty.WriteStream.prototype.hasColors(2));
  assert(tty.WriteStream.prototype.hasColors(2, {}));
});

Deno.test("[node/tty WriteStream.getColorDepth] returns current terminal color depth", () => {
  assert([1, 4, 8, 24].includes(tty.WriteStream.prototype.getColorDepth()));
});

Deno.test("[node/tty isatty] returns false for raw file fd", () => {
  // Open a file and get its raw fd - files are never TTYs
  const fd = fs.openSync("README.md", "r");
  try {
    assert(!isatty(fd), `fd ${fd} should not be a tty`);
  } finally {
    fs.closeSync(fd);
  }
});

Deno.test({
  name: "[node/tty] HandleWrap.close calls uv_close for TTY handles",
  ignore: Deno.build.os === "windows",
  fn: async () => {
    // Verify that destroying a TTY stream closes the underlying fd.
    // We count open fds before and after creating+destroying TTY streams.
    // This catches the bug where HandleWrap.close() did not call
    // uv_compat::uv_close() for Handle::New handles, leaking fds.
    //
    // We spawn a helper that runs under script(1) to ensure fd 0 is a
    // real TTY (not a pipe from the test runner).
    const helper = `
      import * as tty from "node:tty";
      import * as fs from "node:fs";
      import { execSync } from "node:child_process";

      function countFds() {
        // Count open file descriptors for this process
        try {
          return fs.readdirSync("/dev/fd").length;
        } catch {
          try {
            return fs.readdirSync("/proc/self/fd").length;
          } catch { return -1; }
        }
      }

      const before = countFds();

      // Create and destroy TTY WriteStreams. Each should close its
      // internal uv_tty_t handle and fd when destroyed.
      for (let i = 0; i < 10; i++) {
        try {
          const ws = new tty.WriteStream(0);
          ws.destroy();
          await new Promise(r => ws.on("close", r));
        } catch {
          // fd 0 might not be a TTY in CI - skip gracefully
          console.log("SKIP");
          process.exit(0);
        }
      }

      const after = countFds();
      if (before < 0) {
        console.log("SKIP");
        process.exit(0);
      }
      // Allow 1 fd of slack for transient internal use
      if (after > before + 1) {
        console.error("LEAK: before=" + before + " after=" + after);
        process.exit(1);
      }
      console.log("OK");
    `;

    // Use script(1) to give the child a real PTY as its stdin/stdout.
    // Write the helper to a temp file to avoid shell quoting issues.
    // macOS: script -q /dev/null command args...
    // Linux: script -q /dev/null -c "command args..."
    const tmpScript = await Deno.makeTempFile({ suffix: ".mjs" });
    await Deno.writeTextFile(tmpScript, helper);
    const denoCmd =
      `${Deno.execPath()} run --allow-read --allow-run ${tmpScript}`;
    const scriptArgs = Deno.build.os === "linux"
      ? ["-q", "/dev/null", "-c", denoCmd]
      : [
        "-q",
        "/dev/null",
        Deno.execPath(),
        "run",
        "--allow-read",
        "--allow-run",
        tmpScript,
      ];
    const child = new Deno.Command("script", {
      args: scriptArgs,
      stdout: "piped",
      stderr: "piped",
    }).spawn();

    const output = await child.output();
    await Deno.remove(tmpScript);
    const stdout = new TextDecoder().decode(output.stdout);
    const stderr = new TextDecoder().decode(output.stderr);

    if (stdout.includes("SKIP")) {
      return; // Could not get a TTY fd, skip
    }

    assert(
      output.success,
      `TTY fd leak test failed: ${stdout} ${stderr}`,
    );
  },
});
