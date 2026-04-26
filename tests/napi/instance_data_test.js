// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import { assertEquals, loadTestLibrary } from "./common.js";

const properties = loadTestLibrary();

if (import.meta.main) {
  properties.setInstanceData();
  console.log("set instance data");
} else {
  Deno.test("napi instance data Release", async () => {
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
    assertEquals(
      stdoutText,
      "set instance data\ninstance_data_free(42)\n",
    );
  });
}
