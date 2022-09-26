// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

// Note tests for Deno.setRaw is in integration tests.

Deno.test(
  { permissions: { run: true, read: true } },
  async function noColorIfNotTty() {
    const { stdout } = await Deno.spawn(Deno.execPath(), {
      args: ["eval", "console.log(1)"],
    });
    const output = new TextDecoder().decode(stdout);
    assertEquals(output, "1\n");
  },
);
