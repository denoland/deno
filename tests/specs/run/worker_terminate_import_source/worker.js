// Copyright 2018-2026 the Deno authors. MIT license.

Object.defineProperty(WebAssembly.Module.prototype, "then", {
  configurable: true,
  get() {
    postMessage("resolving");
    while (true) {
      // Wait for the parent to terminate this worker.
    }
  },
});

await import.source("../wasm_source_phase_imports_dynamic/math.wasm");
