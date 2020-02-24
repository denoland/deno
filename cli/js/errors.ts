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

export function constructError(kind: ErrorKind, msg: string): never {
  switch (kind) {
    case ErrorKind.TypeError:
      throw new TypeError(msg);
    case ErrorKind.Other:
      throw new Error(msg);
    case ErrorKind.URIError:
      throw new URIError(msg);
    case ErrorKind.NotFound:
      throw new NotFound(msg);
    case ErrorKind.PermissionDenied:
      throw new PermissionDenied(msg);
    case ErrorKind.ConnectionRefused:
      throw new ConnectionRefused(msg);
    case ErrorKind.ConnectionReset:
      throw new ConnectionReset(msg);
    case ErrorKind.ConnectionAborted:
      throw new ConnectionAborted(msg);
    case ErrorKind.NotConnected:
      throw new NotConnected(msg);
    case ErrorKind.AddrInUse:
      throw new AddrInUse(msg);
    case ErrorKind.AddrNotAvailable:
      throw new AddrNotAvailable(msg);
    case ErrorKind.BrokenPipe:
      throw new BrokenPipe(msg);
    case ErrorKind.AlreadyExists:
      throw new AlreadyExists(msg);
    case ErrorKind.InvalidData:
      throw new InvalidData(msg);
    case ErrorKind.TimedOut:
      throw new TimedOut(msg);
    case ErrorKind.Interrupted:
      throw new Interrupted(msg);
    case ErrorKind.WriteZero:
      throw new WriteZero(msg);
    case ErrorKind.UnexpectedEof:
      throw new UnexpectedEof(msg);
    case ErrorKind.BadResource:
      throw new BadResource(msg);
    case ErrorKind.Http:
      throw new Http(msg);
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
