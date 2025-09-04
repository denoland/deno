// Copyright 2018-2025 the Deno authors. MIT license.

import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";

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

export { createTracing, getEnabledCategories };

export default {
  createTracing,
  getEnabledCategories,
};
