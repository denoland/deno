// Copyright 2018-2026 the Deno authors. MIT license.
(function () {
const { core } = globalThis.__bootstrap;
const { ERR_INTERNAL_ASSERTION } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);

function assert(value, message) {
  if (!value) {
    throw new ERR_INTERNAL_ASSERTION(message);
  }
}

function fail(message) {
  throw new ERR_INTERNAL_ASSERTION(message);
}

assert.fail = fail;

return {
  default: assert,
};
})();
