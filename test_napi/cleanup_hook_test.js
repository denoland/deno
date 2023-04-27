// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

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
        "--allow-read",
        "--allow-run",
        "--allow-ffi",
        "--unstable",
        import.meta.url,
      ],
    }).output();

    assertEquals(code, 0);
    assertEquals(new TextDecoder().decode(stderr), "");

    const stdoutText = new TextDecoder().decode(stdout);
    const stdoutLines = stdoutText.split("\n");
    assertEquals(stdoutLines.length, 4);
    assertEquals(stdoutLines[0], "installed cleanup hook");
    assertEquals(stdoutLines[1], "cleanup(18)");
    assertEquals(stdoutLines[2], "cleanup(42)");
  });
}
