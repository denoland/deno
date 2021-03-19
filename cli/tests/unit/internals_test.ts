// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert } from "./test_util.ts";

Deno.test("internalsExists", function (): void {
  const {
    inspectArgs,
    // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
  } = Deno[Deno.internal];
  assert(!!inspectArgs);
});
