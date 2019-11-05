// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assert } from "./test_util.ts";

testPerm({ hrtime: true }, async function now(): Promise<void> {
  const start = performance.now();
  await new Promise((resolve): number => setTimeout(resolve, 10));
  const end = performance.now();
  assert(end - start >= 10);
});
