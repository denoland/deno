// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

/** Assertion error class for node compat layer's internal code. */
export class NodeCompatAssertionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "NodeCompatAssertionError";
  }
}

/** Make an assertion, if not `true`, then throw. */
export function assert(expr: unknown, msg = ""): asserts expr {
  if (!expr) {
    throw new NodeCompatAssertionError(msg);
  }
}

/** Use this to assert unreachable code. */
export function unreachable(): never {
  throw new NodeCompatAssertionError("unreachable");
}
