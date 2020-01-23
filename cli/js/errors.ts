// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** A Deno specific error.  The `kind` property is set to a specific error code
 * which can be used to in application logic.
 *
 *       try {
 *         somethingThatMightThrow();
 *       } catch (e) {
 *         if (
 *           e instanceof Deno.DenoError &&
 *           e.kind === Deno.ErrorKind.NotFound
 *         ) {
 *           console.error("NotFound error!");
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

// Warning! The values in this enum are duplicated in cli/msg.rs
// Update carefully!
export enum ErrorKind {
  NotFound = 1,
  PermissionDenied = 2,
  ConnectionRefused = 3,
  ConnectionReset = 4,
  ConnectionAborted = 5,
  NotConnected = 6,
  AddrInUse = 7,
  AddrNotAvailable = 8,
  BrokenPipe = 9,
  AlreadyExists = 10,
  WouldBlock = 11,
  InvalidInput = 12,
  InvalidData = 13,
  TimedOut = 14,
  Interrupted = 15,
  WriteZero = 16,
  Other = 17,
  UnexpectedEof = 18,
  BadResource = 19,
  UrlParse = 20,
  Http = 21,
  TooLarge = 22,
  InvalidSeekMode = 23,
  UnixError = 24,
  InvalidPath = 25,
  ImportPrefixMissing = 26,
  Diagnostic = 27,
  JSError = 28
}
