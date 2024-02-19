// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { primordials } from "ext:core/mod.js";
import { op_console_size } from "ext:core/ops";
const {
  Uint32Array,
} = primordials;

const size = new Uint32Array(2);

function consoleSize() {
  op_console_size(size);
  return { columns: size[0], rows: size[1] };
}

export { consoleSize };
