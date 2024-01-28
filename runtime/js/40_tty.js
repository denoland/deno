// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { core, internals, primordials } from "ext:core/mod.js";
const {
  op_console_size,
  op_is_terminal,
} = core.ensureFastOps(true);
const {
  Uint32Array,
} = primordials;

const size = new Uint32Array(2);

function consoleSize() {
  op_console_size(size);
  return { columns: size[0], rows: size[1] };
}

function isatty(rid) {
  internals.warnOnDeprecatedApi(
    "Deno.isatty()",
    new Error().stack,
    "Use `Deno.stdin.isTerminal()`, `Deno.stdout.isTerminal()` or `Deno.stderr.isTerminal()` instead.",
  );
  return op_is_terminal(rid);
}

export { consoleSize, isatty };
