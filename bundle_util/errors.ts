// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

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
    this.name = kind;
  }
}

// Warning! The values in this enum are duplicated in cli/msg.rs
// Update carefully!
export type ErrorKind = string;

export enum StandardErrorKinds {
  NoError = "NoError",
  NotFound = "NotFound",
  PermissionDenied = "PermissionDenied",
  ConnectionRefused = "ConnectionRefused",
  ConnectionReset = "ConnectionReset",
  ConnectionAborted = "ConnectionAborted",
  NotConnected = "NotConnected",
  AddrInUse = "AddrInUse",
  AddrNotAvailable = "AddrNotAvailable",
  BrokenPipe = "BrokenPipe",
  AlreadyExists = "AlreadyExists",
  WouldBlock = "WouldBlock",
  InvalidInput = "InvalidInput",
  InvalidData = "InvalidData",
  TimedOut = "TimedOut",
  Interrupted = "Interrupted",
  WriteZero = "WriteZero",
  Other = "Other",
  UnexpectedEof = "UnexpectedEof",
  BadResource = "BadResource",
  CommandFailed = "CommandFailed",
  EmptyHost = "EmptyHost",
  IdnaError = "IdnaError",
  InvalidPort = "InvalidPort",
  InvalidIpv4Address = "InvalidIpv4Address",
  InvalidIpv6Address = "InvalidIpv6Address",
  InvalidDomainCharacter = "InvalidDomainCharacter",
  RelativeUrlWithoutBase = "RelativeUrlWithoutBase",
  RelativeUrlWithCannotBeABaseBase = "RelativeUrlWithCannotBeABaseBase",
  SetHostOnCannotBeABaseUrl = "SetHostOnCannotBeABaseUrl",
  Overflow = "Overflow",
  HttpUser = "HttpUser",
  HttpClosed = "HttpClosed",
  HttpCanceled = "HttpCanceled",
  HttpParse = "HttpParse",
  HttpOther = "HttpOther",
  TooLarge = "TooLarge",
  InvalidUri = "InvalidUri",
  InvalidSeekMode = "InvalidSeekMode",
  OpNotAvailable = "OpNotAvailable",
  WorkerInitFailed = "WorkerInitFailed",
  UnixError = "UnixError",
  NoAsyncSupport = "NoAsyncSupport",
  NoSyncSupport = "NoSyncSupport",
  ImportMapError = "ImportMapError",
  InvalidPath = "InvalidPath",
  ImportPrefixMissing = "ImportPrefixMissing",
  UnsupportedFetchScheme = "UnsupportedFetchScheme",
  TooManyRedirects = "TooManyRedirects",
  Diagnostic = "Diagnostic",
  JSError = "JSError"
};
