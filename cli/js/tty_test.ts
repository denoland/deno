// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert } from "./test_util.ts";

// setRaw test is in integration tests.

// Smoke test to ensure no error.
test(function isTTYSmoke(): void {
  console.log(Deno.isTTY());
});

testPerm({ read: true }, function isatty(): void {
  // CI not under TTY, so cannot test stdin/stdout/stderr.
  const f = Deno.openSync("cli/tests/hello.txt");
  assert(!Deno.isatty(f.rid));
});

test(function isattyError(): void {
  let caught = false;
  try {
    // Absurdly large rid.
    Deno.isatty(0x7fffffff);
  } catch (e) {
    caught = true;
    assert(e instanceof Deno.Err.BadResource);
  }
  assert(caught);
});
