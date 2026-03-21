// Copyright 2018-2026 the Deno authors. MIT license.
import { createInterface, Interface } from "node:readline";
import { assertFalse, assertInstanceOf } from "@std/assert";
import { Readable, Stream, Writable } from "node:stream";

Deno.test("[node/readline] createInstance", () => {
  const rl = createInterface({
    input: new Readable({ read() {} }),
    output: new Writable(),
  });

  // deno-lint-ignore no-explicit-any
  assertInstanceOf(rl, Interface as any);
});

// Test for https://github.com/denoland/deno/issues/19183
Deno.test("[node/readline] don't throw on rl.question()", () => {
  const rli = createInterface({
    input: new Readable({ read() {} }),
    output: new Writable({ write() {} }),
    terminal: true,
  });

  // Calling this would throw
  rli.question("foo", () => rli.close());
  rli.close();
});

Deno.test("[node/readline] createInstance on non-TTY with terminal: true", () => {
  const stream = new Stream() as any;
  stream.isTTY = false;
  stream.resume = function () {};
  stream.pause = function () {};

  let setRawModeCalled = false;
  stream.setRawMode = function () {
    setRawModeCalled = true;
    throw new Error("setRawMode should not be called for non-TTY streams");
  };

  const rl = createInterface({
    input: stream,
    output: stream,
    terminal: true,
  });
  rl.close();
  assertFalse(setRawModeCalled);
});
