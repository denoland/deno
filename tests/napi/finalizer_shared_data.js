// Copyright 2018-2026 the Deno authors. MIT license.

// Regression test for #36538. All the externals created here share the same
// (null) `data` pointer while each carries its own finalize hint, which is the
// shape addons such as `@duckdb/node-api` produce. The runtime used to identify
// pending finalizers by their `data` pointer, so collecting one external
// deregistered an unrelated external's finalizer; the collected one's finalizer
// was then called a second time during shutdown, with an already freed hint,
// crashing the process with SIGSEGV/SIGBUS.

import { assert, assertEquals, loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

const kept = [];
for (let i = 0; i < 200; i++) {
  const external = lib.test_shared_data_external(i);
  // Keep a quarter of them reachable, so that they are only finalized at
  // shutdown while the rest are finalized by the garbage collector.
  if (i % 4 === 0) {
    kept.push(external);
  }
}

for (let i = 0; i < 3; i++) {
  globalThis.gc();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

assert(
  lib.test_shared_data_finalized_count() > 0,
  "expected the garbage collector to finalize some externals",
);
assertEquals(
  lib.test_shared_data_double_count(),
  0,
  "no finalizer may run more than once",
);

// Keep the remaining externals alive until shutdown; finalizing them must not
// re-run the finalizers of the ones already collected above.
globalThis.keptExternals = kept;
// deno-lint-ignore no-console
console.log("shared data externals finalized once");
