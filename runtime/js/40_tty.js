// Copyright 2018-2026 the Deno authors. MIT license.
(function () {
const { core, primordials } = globalThis.__bootstrap;
const { op_console_size } = core.ops;
const {
  Uint32Array,
} = primordials;
const {
  isTerminal,
} = core;

const size = new Uint32Array(2);

function consoleSize() {
  op_console_size(size);
  return { columns: size[0], rows: size[1] };
}

// Note: This function was soft-removed in Deno 2. Its types have been removed,
// but its implementation has been kept to avoid breaking changes.
function isatty(rid) {
  return isTerminal(rid);
}

return { consoleSize, isatty };
})();
