// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

import { assertEquals, loadTestLibrary } from "./common.js";

const properties = loadTestLibrary();

if (import.meta.main) {
  properties.installCleanupHook();
  console.log("installed cleanup hook");
} else {
  Deno.test("napi cleanup hook", async () => {
    const { stdout, stderr, code } = await new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--config",
        Deno.realPathSync("../config/deno.json"),
        "--no-lock",
        "-A",
        "--unstable-ffi",
        import.meta.url,
      ],
    }).output();

    assertEquals(new TextDecoder().decode(stderr), "");
    assertEquals(code, 0);

    const stdoutText = new TextDecoder().decode(stdout);
    const stdoutLines = stdoutText.split("\n");
    assertEquals(stdoutLines.length, 4);
    assertEquals(stdoutLines[0], "installed cleanup hook");
    assertEquals(stdoutLines[1], "cleanup(18)");
    assertEquals(stdoutLines[2], "cleanup(42)");
  });
}
