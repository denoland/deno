// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertThrows, unitTest } from "./test_util.ts";

// Note tests for Deno.setRaw is in integration tests.

unitTest({ perms: { read: true } }, function consoleSizeFile() {
  const file = Deno.openSync("cli/tests/testdata/hello.txt");
  assertThrows(() => {
    Deno.consoleSize(file.rid);
  }, Error);
  file.close();
});

unitTest(function consoleSizeError() {
  assertThrows(() => {
    // Absurdly large rid.
    Deno.consoleSize(0x7fffffff);
  }, Deno.errors.BadResource);
});

unitTest({ perms: { read: true } }, function isatty() {
  // CI not under TTY, so cannot test stdin/stdout/stderr.
  const f = Deno.openSync("cli/tests/testdata/hello.txt");
  assert(!Deno.isatty(f.rid));
  f.close();
});

unitTest(function isattyError() {
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
