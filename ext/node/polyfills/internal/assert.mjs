// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
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

export default assert;
