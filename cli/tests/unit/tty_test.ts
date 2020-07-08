// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

// Note tests for Deno.setRaw is in integration tests.

unitTest(function getConsoleSizeFile(): void {
  const file = Deno.openSync("cli/tests/hello.txt");
  assertThrows(() => {
    Deno.getConsoleSize(file.rid);
  }, Deno.errors.Other);
});

unitTest(function getConsoleSizeError(): void {
  let caught = false;
  try {
    // Absurdly large rid.
    Deno.getConsoleSize(0x7fffffff);
  } catch (e) {
    caught = true;
    assert(e instanceof Deno.errors.BadResource);
  }
  assert(caught);
});

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
