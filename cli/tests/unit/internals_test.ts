// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

unitTest(function internalsExists(): void {
  const {
    stringifyArgs,
    // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
  } = Deno[Deno.internal];
  assert(!!stringifyArgs);
});
