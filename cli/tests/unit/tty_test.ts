// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertThrows } from "./test_util.ts";

// Note tests for Deno.setRaw is in integration tests.

Deno.test("consoleSizeFile", function (): void {
  const file = Deno.openSync("cli/tests/hello.txt");
  assertThrows(() => {
    Deno.consoleSize(file.rid);
  }, Error);
  file.close();
});

Deno.test("consoleSizeError", function (): void {
  assertThrows(() => {
    // Absurdly large rid.
    Deno.consoleSize(0x7fffffff);
  }, Deno.errors.BadResource);
});

Deno.test("isatty", function (): void {
  // CI not under TTY, so cannot test stdin/stdout/stderr.
  const f = Deno.openSync("cli/tests/hello.txt");
  assert(!Deno.isatty(f.rid));
  f.close();
});

Deno.test("isattyError", function (): void {
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
