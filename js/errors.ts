// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { ErrorKind } from "gen/cli/msg_generated";
export { ErrorKind } from "gen/cli/msg_generated";

/** A Deno specific error.  The `kind` property is set to a specific error code
 * which can be used to in application logic.
 *
 *       try {
 *         somethingThatMightThrow();
 *       } catch (e) {
 *         if (
 *           e instanceof Deno.DenoError &&
 *           e.kind === Deno.ErrorKind.Overflow
 *         ) {
 *           console.error("Overflow error!");
 *         }
 *       }
 *
 */
export class DenoError<T extends ErrorKind> extends Error {
  constructor(readonly kind: T, msg: string) {
    super(msg);
    this.name = ErrorKind[kind];
  }
}
