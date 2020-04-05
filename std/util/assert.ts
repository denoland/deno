/*
  NOTE: DO NOT import any module into this file
  This file was separated from asserts.ts because it makes bunled size heavy
  https://github.com/denoland/deno/issues/3933
*/

export class AssertionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AssertionError";
  }
}

/** Make an assertion, if not `true`, then throw. */
export function assert(expr: unknown, msg = ""): asserts expr {
  if (!expr) {
    throw new AssertionError(msg);
  }
}
