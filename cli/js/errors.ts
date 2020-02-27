// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Warning! The values in this enum are duplicated in cli/op_error.rs
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
  InvalidData = 13,
  TimedOut = 14,
  Interrupted = 15,
  WriteZero = 16,
  UnexpectedEof = 17,
  BadResource = 18,
  Http = 19,
  URIError = 20,
  TypeError = 21,
  Other = 22
}

export function constructError(kind: ErrorKind, msg: string): Error {
  switch (kind) {
    case ErrorKind.TypeError:
      return new TypeError(msg);
    case ErrorKind.Other:
      return new Error(msg);
    case ErrorKind.URIError:
      return new URIError(msg);
    case ErrorKind.NotFound:
      return new NotFound(msg);
    case ErrorKind.PermissionDenied:
      return new PermissionDenied(msg);
    case ErrorKind.ConnectionRefused:
      return new ConnectionRefused(msg);
    case ErrorKind.ConnectionReset:
      return new ConnectionReset(msg);
    case ErrorKind.ConnectionAborted:
      return new ConnectionAborted(msg);
    case ErrorKind.NotConnected:
      return new NotConnected(msg);
    case ErrorKind.AddrInUse:
      return new AddrInUse(msg);
    case ErrorKind.AddrNotAvailable:
      return new AddrNotAvailable(msg);
    case ErrorKind.BrokenPipe:
      return new BrokenPipe(msg);
    case ErrorKind.AlreadyExists:
      return new AlreadyExists(msg);
    case ErrorKind.InvalidData:
      return new InvalidData(msg);
    case ErrorKind.TimedOut:
      return new TimedOut(msg);
    case ErrorKind.Interrupted:
      return new Interrupted(msg);
    case ErrorKind.WriteZero:
      return new WriteZero(msg);
    case ErrorKind.UnexpectedEof:
      return new UnexpectedEof(msg);
    case ErrorKind.BadResource:
      return new BadResource(msg);
    case ErrorKind.Http:
      return new Http(msg);
  }
}

class NotFound extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "NotFound";
  }
}
class PermissionDenied extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "PermissionDenied";
  }
}
class ConnectionRefused extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "ConnectionRefused";
  }
}
class ConnectionReset extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "ConnectionReset";
  }
}
class ConnectionAborted extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "ConnectionAborted";
  }
}
class NotConnected extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "NotConnected";
  }
}
class AddrInUse extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "AddrInUse";
  }
}
class AddrNotAvailable extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "AddrNotAvailable";
  }
}
class BrokenPipe extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "BrokenPipe";
  }
}
class AlreadyExists extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "AlreadyExists";
  }
}
class InvalidData extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "InvalidData";
  }
}
class TimedOut extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "TimedOut";
  }
}
class Interrupted extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "Interrupted";
  }
}
class WriteZero extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "WriteZero";
  }
}
class Other extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "Other";
  }
}
class UnexpectedEof extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "UnexpectedEof";
  }
}
class BadResource extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "BadResource";
  }
}
class Http extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = "Http";
  }
}

export const errors = {
  NotFound: NotFound,
  PermissionDenied: PermissionDenied,
  ConnectionRefused: ConnectionRefused,
  ConnectionReset: ConnectionReset,
  ConnectionAborted: ConnectionAborted,
  NotConnected: NotConnected,
  AddrInUse: AddrInUse,
  AddrNotAvailable: AddrNotAvailable,
  BrokenPipe: BrokenPipe,
  AlreadyExists: AlreadyExists,
  InvalidData: InvalidData,
  TimedOut: TimedOut,
  Interrupted: Interrupted,
  WriteZero: WriteZero,
  Other: Other,
  UnexpectedEof: UnexpectedEof,
  BadResource: BadResource,
  Http: Http
};
