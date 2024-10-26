// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { core, primordials } from "ext:core/mod.js";
import { op_console_size } from "ext:core/ops";
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

export { consoleSize, isatty };
