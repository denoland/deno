// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { core } from "deno:core/01_core.js";
import primordials from "deno:core/00_primordials.js";
const {
  Uint32Array,
  Uint8Array,
} = primordials;
const ops = core.ops;

const size = new Uint32Array(2);

function consoleSize() {
  ops.op_console_size(size);
  return { columns: size[0], rows: size[1] };
}

const isattyBuffer = new Uint8Array(1);
function isatty(rid) {
  ops.op_isatty(rid, isattyBuffer);
  return !!isattyBuffer[0];
}

export { consoleSize, isatty };
