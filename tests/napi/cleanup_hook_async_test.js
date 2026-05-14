// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import { assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

if (Deno.args[0] === "install") {
  lib.installAsyncCleanupHooks();
  console.log("installed async cleanup hooks");
} else if (Deno.args[0] === "install_remove") {
  lib.installAndRemoveAsyncCleanupHook();
  console.log("installed and removed async cleanup hook");
} else {
  Deno.test("napi async cleanup hooks are called on exit", async () => {
    const { stdout, stderr, code } = await new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--config",
        Deno.realPathSync("../config/deno.json"),
        "--no-lock",
        "-A",
        "--unstable-ffi",
        import.meta.url,
        "install",
      ],
    }).output();

    assertEquals(new TextDecoder().decode(stderr), "");
    assertEquals(code, 0);

    const lines = new TextDecoder().decode(stdout).split("\n");
    assertEquals(lines[0], "installed async cleanup hooks");
    // Async cleanup hooks fire in LIFO order
    assertEquals(lines[1], "async_cleanup(20)");
    assertEquals(lines[2], "async_cleanup(10)");
  });

  Deno.test(
    "napi removed async cleanup hook is not called on exit",
    async () => {
      const { stdout, stderr, code } = await new Deno.Command(
        Deno.execPath(),
        {
          args: [
            "run",
            "--config",
            Deno.realPathSync("../config/deno.json"),
            "--no-lock",
            "-A",
            "--unstable-ffi",
            import.meta.url,
            "install_remove",
          ],
        },
      ).output();

      assertEquals(new TextDecoder().decode(stderr), "");
      assertEquals(code, 0);

      const lines = new TextDecoder().decode(stdout).split("\n");
      assertEquals(lines[0], "installed and removed async cleanup hook");
      // The hook with value 99 should NOT appear
      assertEquals(lines.filter((l) => l.includes("async_cleanup")).length, 0);
    },
  );
}
