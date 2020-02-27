// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { errors } from "../errors.ts";
const { AssertionError } = errors;

/** Make an assertion, if not `true`, then throw. */
export function assert(expr: unknown, msg = ""): asserts expr {
  if (!expr) {
    throw new AssertionError(msg);
  }
}
