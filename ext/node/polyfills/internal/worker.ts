// Copyright 2018-2026 the Deno authors. MIT license.

// Stub for Node's `internal/worker` module. Only the symbols actually
// reached for by tests in `tests/node_compat` are exposed; the rest of
// Node's private API surface (Worker class internals, kIsOnline, ...)
// is intentionally omitted.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { MapPrototype, MapPrototypeForEach, ObjectPrototypeIsPrototypeOf } =
  primordials;
const workerThreads = core.loadExtScript(
  "ext:deno_node/worker_threads.ts",
);

// `assignEnvironmentData` in Node merges a Map into the per-thread
// environment data. The public `setEnvironmentData` only supports
// key/value pairs; we map this onto the same backing store by either
// no-op'ing for `undefined` (Node behavior) or copying the Map.
function assignEnvironmentData(data) {
  if (data === undefined || data === null) return;
  if (
    typeof data === "object" &&
    ObjectPrototypeIsPrototypeOf(MapPrototype, data)
  ) {
    MapPrototypeForEach(data, (value, key) => {
      workerThreads.setEnvironmentData(key, value);
    });
  }
}

return {
  assignEnvironmentData,
};
})();
