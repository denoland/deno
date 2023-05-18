// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { createInterface, Interface } from "node:readline";
import { assertInstanceOf } from "../../../test_util/std/testing/asserts.ts";
import { Readable, Writable } from "node:stream";

Deno.test("[node/readline] createInstance", () => {
  const rl = createInterface({
    input: new Readable({ read() {} }),
    output: new Writable(),
  });

  // deno-lint-ignore no-explicit-any
  assertInstanceOf(rl, Interface as any);
});
