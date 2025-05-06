// Copyright 2018-2025 the Deno authors. MIT license.
// @deno-types="./_readline.d.ts"

import {
  clearLine,
  clearScreenDown,
  createInterface,
  cursorTo,
  emitKeypressEvents,
  Interface,
  moveCursor,
  promises,
} from "ext:deno_node/_readline.mjs";

export {
  clearLine,
  clearScreenDown,
  createInterface,
  cursorTo,
  emitKeypressEvents,
  Interface,
  moveCursor,
  promises,
};

export default {
  Interface,
  clearLine,
  clearScreenDown,
  createInterface,
  cursorTo,
  emitKeypressEvents,
  moveCursor,
  promises,
};
