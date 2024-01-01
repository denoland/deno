// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any

import { assert } from "../../../test_util/std/assert/mod.ts";
import { isatty } from "node:tty";

Deno.test("[node/tty isatty] returns true when fd is a tty, false otherwise", () => {
  assert(Deno.isatty(Deno.stdin.rid) === isatty(Deno.stdin.rid));
  assert(Deno.isatty(Deno.stdout.rid) === isatty(Deno.stdout.rid));
  assert(Deno.isatty(Deno.stderr.rid) === isatty(Deno.stderr.rid));

  const file = Deno.openSync("README.md");
  assert(!isatty(file.rid));
  Deno.close(file.rid);
});

Deno.test("[node/tty isatty] returns false for irrelevant values", () => {
  // invalid numeric fd
  assert(!isatty(1234567));

  // TODO(kt3k): Enable this test when the below issue resolved
  // https://github.com/denoland/deno/issues/14398
  // assert(!isatty(-1));

  // invalid type fd
  assert(!isatty("abc" as any));
  assert(!isatty({} as any));
  assert(!isatty([] as any));
  assert(!isatty(null as any));
  assert(!isatty(undefined as any));
});
