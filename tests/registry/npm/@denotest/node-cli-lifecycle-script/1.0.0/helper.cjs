// Preloaded via `node --require ./helper.cjs`. Records that the preload ran (and
// that it ran under Deno) so `main.cjs` can observe it through the shared realm.
globalThis.__node_require_preload = typeof Deno;
