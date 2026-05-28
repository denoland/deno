// Copyright 2018-2026 the Deno authors. MIT license.

// This file re-exports the pure JS KV implementation.
// The implementation lives in impl/kv.ts and its dependencies.
//
// Kept as a separate ESM entry so that runtime/js/90_deno_ns.js can lazy-load
// the KV namespace via the existing "ext:deno_kv/01_db.ts" specifier without
// churn in the runtime wiring.

export {
  AtomicOperation,
  Kv,
  KvListIterator,
  KvU64,
  openKv,
} from "ext:deno_kv/impl/kv.ts";
