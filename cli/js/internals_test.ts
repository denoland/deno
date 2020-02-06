// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function internalsExists(): void {
  const {
    stringifyArgs
    // @ts-ignore
  } = Deno[Deno.symbols.internal];
  assert(!!stringifyArgs);
});
