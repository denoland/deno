// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { Console } = core.loadExtScript(
  "ext:deno_node/internal/console/constructor.mjs",
);
const { windowOrWorkerGlobalScope } = core.loadExtScript(
  "ext:runtime/98_global_scope_shared.js",
);
// Don't rely on global `console` because during bootstrapping, it is pointing
// to native `console` object provided by V8.
const console = windowOrWorkerGlobalScope.console.value;

const { ObjectAssign } = primordials;

ObjectAssign(console, { Console });

return console;
})();
