// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = globalThis.__bootstrap;
const { op_inspector_enabled } = core.ops;

function isEnabled() {
  return op_inspector_enabled();
}

return {
  isEnabled,
};
})();
