// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "./test_util.ts";

// Note tests for Deno.stdin.setRaw is in integration tests.

Deno.test(function consoleSize() {
  if (!Deno.isatty(Deno.stdout.rid)) {
    return;
  }
  const result = Deno.consoleSize();
  assert(typeof result.columns !== "undefined");
  assert(typeof result.rows !== "undefined");
});

Deno.test({ permissions: { read: true } }, function isatty() {
  // CI not under TTY, so cannot test stdin/stdout/stderr.
  const f = Deno.openSync("cli/tests/testdata/assets/hello.txt");
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
