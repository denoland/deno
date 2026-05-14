// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = globalThis.__bootstrap;
const { Console } = core.loadExtScript(
  "ext:deno_node/internal/console/constructor.mjs",
);

return { Console };
})();
