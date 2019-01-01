// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { Base, ErrorKind } from "gen/msg_generated";
export { ErrorKind } from "gen/msg_generated";

/** A Deno specific error.  The `kind` property is set to a specific error code
 * which can be used to in application logic.
 *
 *     import { DenoError, ErrorKind } from "deno";
 *     try {
 *       somethingThatMightThrow();
 *     } catch (e) {
 *       if (e instanceof DenoError && e.kind === ErrorKind.Overflow) {
 *         console.error("Overflow error!");
 *       }
 *     }
 */
export class DenoError<T extends ErrorKind> extends Error {
  constructor(readonly kind: T, msg: string) {
    super(msg);
    this.name = ErrorKind[kind];
  }
}

// @internal
export function maybeThrowError(base: Base): void {
  const err = maybeError(base);
  if (err != null) {
    throw err;
  }
}

// @internal
export function maybeError(base: Base): null | DenoError<ErrorKind> {
  const kind = base.errorKind();
  if (kind === ErrorKind.NoError) {
    return null;
  } else {
    return new DenoError(kind, base.error()!);
  }
}
