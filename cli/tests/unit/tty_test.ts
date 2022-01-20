// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assert, assertThrows } from "./test_util.ts";

// Note tests for Deno.setRaw is in integration tests.

Deno.test({ permissions: { read: true } }, function consoleSizeFile() {
  const file = Deno.openSync("cli/tests/testdata/hello.txt");
  assertThrows(() => {
    Deno.consoleSize(file.rid);
  }, Error);
  file.close();
});

Deno.test(function consoleSizeError() {
  assertThrows(() => {
    // Absurdly large rid.
    Deno.consoleSize(0x7fffffff);
  }, Deno.errors.BadResource);
});

Deno.test({ permissions: { read: true } }, function isatty() {
  // CI not under TTY, so cannot test stdin/stdout/stderr.
  const f = Deno.openSync("cli/tests/testdata/hello.txt");
  assert(!Deno.isatty(f.rid));
  f.close();
});

Deno.test(function isattyError() {
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
