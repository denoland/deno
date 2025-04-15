// Copyright 2018-2025 the Deno authors. MIT license.
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

Deno.test(
  { permissions: { run: true, read: true } },
  async function denoNoColorIsNotAffectedByNonTty() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log(Deno.noColor)"],
    }).output();
    const output = new TextDecoder().decode(stdout);
    assertEquals(output, "false\n");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function denoNoColorTrueEmptyVar() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log(Deno.noColor)"],
      env: {
        // https://no-color.org/ -- should not be true when empty
        NO_COLOR: "",
      },
    }).output();
    const output = new TextDecoder().decode(stdout);
    assertEquals(output, "false\n");
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function denoNoColorTrueEmptyVar() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log(Deno.noColor)"],
      env: {
        NO_COLOR: "1",
      },
    }).output();
    const output = new TextDecoder().decode(stdout);
    assertEquals(output, "true\n");
  },
);
