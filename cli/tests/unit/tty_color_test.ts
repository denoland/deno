// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

// Note tests for Deno.setRaw is in integration tests.

Deno.test(
  { permissions: { run: true, read: true } },
  async function noColorIfNotTty() {
    const p = Deno.run({
      cmd: [Deno.execPath(), "eval", "console.log(1)"],
      stdout: "piped",
    });
    const output = new TextDecoder().decode(await p.output());
    assertEquals(output, "1\n");
    p.close();
  },
);
