// Copyright 2018-2026 the Deno authors. MIT license.

// Eagerly register structured-clone deserializers for node host objects
// (crypto KeyObject/X509Certificate, perf_hooks histograms).
//
// These used to be registered as a side effect of evaluating their (lazy)
// polyfill modules. Under node-defer those modules are not evaluated at worker
// startup, so a worker that received one of these objects via
// postMessage/workerData had no deserializer registered and crashed in
// `read_host_object`. This module is evaluated eagerly (loaded from
// `98_global_scope_shared.js`, so the registrations bake into the snapshot and
// are present in every worker), while the actual deserialization impl is still
// loaded lazily on first transfer.

(function () {
const { core } = __bootstrap;

let keysMod;
const lazyKeys = () =>
  keysMod ??
    (keysMod = core.loadExtScript("ext:deno_node/internal/crypto/keys.ts"));
let x509Mod;
const lazyX509 = () =>
  x509Mod ??
    (x509Mod = core.loadExtScript("ext:deno_node/internal/crypto/x509.ts"));
let perfHooksMod;
const lazyPerfHooks = () =>
  perfHooksMod ??
    (perfHooksMod = core.loadExtScript("ext:deno_node/perf_hooks.js"));

core.registerCloneableResource(
  "NodeCryptoKeyObject",
  (data) => lazyKeys().deserializeNodeCryptoKeyObject(data),
);
core.registerCloneableResource(
  "X509Certificate",
  (data) => lazyX509().deserializeX509Certificate(data),
);
core.registerCloneableResource(
  "EventLoopDelayHistogram",
  (data) => lazyPerfHooks().snapshotHistogram(data),
);
core.registerCloneableResource(
  "RecordableHistogram",
  (data) => lazyPerfHooks().deserializeRecordableHistogram(data),
);
})();
