// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any

import { assert } from "@std/assert";
import { isatty } from "node:tty";
import tty from "node:tty";
import process from "node:process";
import fs from "node:fs";

Deno.test("[node/tty isatty] returns true when fd is a tty, false otherwise", () => {
  // Uses raw file descriptors: 0 = stdin, 1 = stdout, 2 = stderr
  assert(Deno.stdin.isTerminal() === isatty(0));
  assert(Deno.stdout.isTerminal() === isatty(1));
  assert(Deno.stderr.isTerminal() === isatty(2));
});

Deno.test("[node/tty isatty] returns false for irrelevant values", () => {
  // invalid numeric fd
  assert(!isatty(1234567));

  // negative fd should return false
  assert(!isatty(-1));

  // non-integer numeric fd should return false
  assert(!isatty(0.5));
  assert(!isatty(1.3));
  assert(!isatty(2.2));
  assert(!isatty(3.1));

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

Deno.test("[node/tty isatty] returns false for raw file fd", () => {
  // Open a file and get its raw fd - files are never TTYs
  const fd = fs.openSync("README.md", "r");
  try {
    assert(!isatty(fd), `fd ${fd} should not be a tty`);
  } finally {
    fs.closeSync(fd);
  }
});
