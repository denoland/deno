// Copyright 2018-2026 the Deno authors. MIT license.

// Backs `process.binding('inspector')`. Node exposes a handful of C++ helpers
// here; we only implement what user-visible code reaches for.

(function () {
const { core } = globalThis.__bootstrap;
const { op_inspector_enabled } = core.ops;

function isEnabled(): boolean {
  return op_inspector_enabled();
}

const _defaultExport = {
  isEnabled,
};

return {
  isEnabled,
  default: _defaultExport,
};
})();
