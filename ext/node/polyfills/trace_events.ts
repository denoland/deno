// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = globalThis.__bootstrap;
const { ERR_INVALID_ARG_TYPE } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);

class Tracing {
  enabled = false;
  categories = "";
}

function createTracing(opts) {
  if (typeof opts !== "object" || opts == null) {
    throw new ERR_INVALID_ARG_TYPE("options", "Object", opts);
  }

  return new Tracing(opts);
}

function getEnabledCategories() {
  return "";
}

return {
  default: {
    createTracing,
    getEnabledCategories,
  },
  createTracing,
  getEnabledCategories,
};
})();
