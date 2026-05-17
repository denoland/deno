// Copyright 2018-2026 the Deno authors. MIT license.

// Stub for Node's `internal/worker` module. Only the symbols actually
// reached for by tests in `tests/node_compat` are exposed; the rest of
// Node's private API surface (Worker class internals, kIsOnline, ...)
// is intentionally omitted.

(function () {
const workerThreads = globalThis.__bootstrap.core.loadExtScript(
  "ext:deno_node/worker_threads.ts",
);

// `assignEnvironmentData` in Node merges a Map into the per-thread
// environment data. The public `setEnvironmentData` only supports
// key/value pairs; we map this onto the same backing store by either
// no-op'ing for `undefined` (Node behavior) or iterating the supplied
// map/iterable.
function assignEnvironmentData(data) {
  if (data === undefined || data === null) return;
  if (typeof data.entries === "function") {
    for (const entry of data.entries()) {
      workerThreads.setEnvironmentData(entry[0], entry[1]);
    }
  }
}

return {
  assignEnvironmentData,
};
})();
