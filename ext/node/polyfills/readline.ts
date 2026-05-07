// Copyright 2018-2026 the Deno authors. MIT license.
// @deno-types="./_readline.d.ts"

(function () {
const { core } = globalThis.__bootstrap;
const {
  clearLine,
  clearScreenDown,
  createInterface,
  cursorTo,
  emitKeypressEvents,
  Interface,
  moveCursor,
  promises,
} = core.loadExtScript("ext:deno_node/_readline.mjs");

return {
  clearLine,
  clearScreenDown,
  createInterface,
  cursorTo,
  emitKeypressEvents,
  Interface,
  moveCursor,
  promises,
};
})();
