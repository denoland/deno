// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any

import { assert } from "@std/assert";
import { isatty } from "node:tty";
import tty from "node:tty";
import process from "node:process";

Deno.test("[node/tty isatty] returns true when fd is a tty, false otherwise", () => {
  assert(Deno.stdin.isTerminal() === isatty((Deno as any).stdin.rid));
  assert(Deno.stdout.isTerminal() === isatty((Deno as any).stdout.rid));
  assert(Deno.stderr.isTerminal() === isatty((Deno as any).stderr.rid));
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

Deno.test("[node/tty WriteStream.isTTY] returns true when fd is a tty", () => {
  assert(Deno.stdin.isTerminal() === process.stdin.isTTY);
  assert(Deno.stdout.isTerminal() === process.stdout.isTTY);
});

Deno.test("[node/tty WriteStream.hasColors] returns true when colors are supported", () => {
  assert(tty.WriteStream.prototype.hasColors() === !Deno.noColor);
  assert(tty.WriteStream.prototype.hasColors({}) === !Deno.noColor);

  assert(tty.WriteStream.prototype.hasColors(1));
  assert(tty.WriteStream.prototype.hasColors(1, {}));
});

Deno.test("[node/tty WriteStream.getColorDepth] returns current terminal color depth", () => {
  assert([1, 4, 8, 24].includes(tty.WriteStream.prototype.getColorDepth()));
});
