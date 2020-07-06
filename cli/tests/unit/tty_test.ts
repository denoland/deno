// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertStringContains } from "./test_util.ts";

// Note tests for Deno.setRaw is in integration tests.

unitTest({ perms: { read: true } }, function isatty(): void {
  // CI not under TTY, so cannot test stdin/stdout/stderr.
  const f = Deno.openSync("cli/tests/hello.txt");
  assert(!Deno.isatty(f.rid));
  f.close();
});

unitTest(function isattyError(): void {
  let caught = false;
  try {
    // Absurdly large rid.
    Deno.isatty(0x7fffffff);
  } catch (e) {
    caught = true;
    assert(e instanceof Deno.errors.BadResource);
  }
  assert(caught);
});

unitTest(
  { perms: { read: true, run: true }, ignore: Deno.build.os === "windows" },
  async function setRawShouldNotPanicOnNoTTYContext(): Promise<void> {
    // issue #6604
    const decoder = new TextDecoder();
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "run",
        "--unstable",
        "cli/tests/raw_mode_on_notty.ts",
      ],
      stdin: "piped",
      stderr: "piped",
    });
    const output = await p.stderrOutput();
    p.stdin!.close();
    p.close();
    assertStringContains(decoder.decode(output), "ENOTTY");
  }
);
