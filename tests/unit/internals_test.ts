// Copyright 2018-2026 the Deno authors. MIT license.
import { assert } from "./test_util.ts";

Deno.test(function internalsExists() {
  const {
    inspectArgs,
    // @ts-expect-error TypeScript (as of 3.7) does not support indexing namespaces by symbol
  } = Deno[Deno.internal];
  assert(!!inspectArgs);
});

Deno.test(function upgradeHttpRawIsNotExposed() {
  // @ts-expect-error TypeScript does not support indexing namespaces by symbol
  assert(!("upgradeHttpRaw" in Deno[Deno.internal]));
});
