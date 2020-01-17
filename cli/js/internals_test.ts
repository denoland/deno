// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function internalsExists(): void {
  const {
    stringifyArgs
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
  } = Deno[Deno.symbols.internal] as any;
  assert(!!stringifyArgs);
});
