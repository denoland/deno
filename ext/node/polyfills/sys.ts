// Copyright 2018-2026 the Deno authors. MIT license.
(function () {
const { core } = globalThis.__bootstrap;
return core.createLazyLoader("node:util")();
})();
