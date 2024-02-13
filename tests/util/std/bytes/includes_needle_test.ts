// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { includesNeedle } from "./includes_needle.ts";
import { assert } from "../assert/mod.ts";

Deno.test("[bytes] includesNeedle", () => {
  const encoder = new TextEncoder();
  const source = encoder.encode("deno.land");
  const pattern = encoder.encode("deno");

  assert(includesNeedle(source, pattern));
  assert(includesNeedle(new Uint8Array([0, 1, 2, 3]), new Uint8Array([2, 3])));

  assert(includesNeedle(source, pattern, -10));
  assert(!includesNeedle(source, pattern, -1));
});
