// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { createInterface, Interface } from "node:readline";
import { assertInstanceOf } from "../../../test_util/std/assert/mod.ts";
import { Readable, Writable } from "node:stream";

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
