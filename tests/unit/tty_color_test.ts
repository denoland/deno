// Copyright 2018-2025 the Deno authors. MIT license.
import { assert, assertEquals } from "./test_util.ts";

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

Deno.test(
  { permissions: { run: true, read: true } },
  async function denoColorDepth() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log(Deno.colorDepth)"],
    }).output();
    const output = new TextDecoder().decode(stdout).trim();
    assert([1, 4, 8, 24].includes(+output));
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function denoColorDepthDisabled() {
    const { stdout } = await new Deno.Command(Deno.execPath(), {
      args: ["eval", "console.log(Deno.colorDepth)"],
      env: {
        NO_COLOR: "1",
      },
    }).output();
    const output = new TextDecoder().decode(stdout).trim();
    assert([1, 4, 8, 24].includes(+output));
  },
);
