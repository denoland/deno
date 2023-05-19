// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, loadTestLibrary } from "./common.js";

if (import.meta.main) {
  const instanceData = loadTestLibrary();
  instanceData.setPrintOnDelete();
  assertEquals(instanceData.increment(), 42);
  let callCount = 0;
  instanceData.objectWithFinalizer(() => callCount++);
  console.log("created object");
  gc();
  console.log("gc run");
  assertEquals(callCount, 1);
} else {
  Deno.test("napi instance data", async () => {
    const { stdout, stderr, code } = await new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--allow-read",
        "--allow-run",
        "--allow-ffi",
        "--v8-flags=--expose-gc",
        "--unstable",
        import.meta.url,
      ],
    }).output();

    assertEquals(code, 0);
    assertEquals(new TextDecoder().decode(stderr), "");

    const stdoutText = new TextDecoder().decode(stdout);
    const stdoutLines = stdoutText.split("\n");
    assertEquals(stdoutLines[0], "deleting addon data");
  });
}
