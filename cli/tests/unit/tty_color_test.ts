// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

// Note tests for Deno.stdin.setRaw is in integration tests.

Deno.test(
  { permissions: { run: true, read: true } },
  async function noColorIfNotTty() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log(1)"],
    }).output();
    const output = new TextDecoder().decode(stdout);
    assertEquals(output, "1\n");
  },
);
