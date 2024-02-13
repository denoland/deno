// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError } from "./assertion_error.ts";

type Falsy = false | 0 | 0n | "" | null | undefined;

/** Make an assertion, error will be thrown if `expr` have truthy value. */
export function assertFalse(expr: unknown, msg = ""): asserts expr is Falsy {
  if (expr) {
    throw new AssertionError(msg);
  }
}
