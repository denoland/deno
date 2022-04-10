// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

export class DenoStdInternalError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DenoStdInternalError";
  }
}

/** Make an assertion, if not `true`, then throw. */
export function assert(expr: unknown, msg = ""): asserts expr {
  if (!expr) {
    throw new DenoStdInternalError(msg);
  }
}
