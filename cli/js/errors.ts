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
  Other = 22,
}

export function getErrorClass(kind: ErrorKind): { new (msg: string): Error } {
  switch (kind) {
    case ErrorKind.TypeError:
      return TypeError;
    case ErrorKind.Other:
      return Error;
    case ErrorKind.URIError:
      return URIError;
    case ErrorKind.NotFound:
      return NotFound;
    case ErrorKind.PermissionDenied:
      return PermissionDenied;
    case ErrorKind.ConnectionRefused:
      return ConnectionRefused;
    case ErrorKind.ConnectionReset:
      return ConnectionReset;
    case ErrorKind.ConnectionAborted:
      return ConnectionAborted;
    case ErrorKind.NotConnected:
      return NotConnected;
    case ErrorKind.AddrInUse:
      return AddrInUse;
    case ErrorKind.AddrNotAvailable:
      return AddrNotAvailable;
    case ErrorKind.BrokenPipe:
      return BrokenPipe;
    case ErrorKind.AlreadyExists:
      return AlreadyExists;
    case ErrorKind.InvalidData:
      return InvalidData;
    case ErrorKind.TimedOut:
      return TimedOut;
    case ErrorKind.Interrupted:
      return Interrupted;
    case ErrorKind.WriteZero:
      return WriteZero;
    case ErrorKind.UnexpectedEof:
      return UnexpectedEof;
    case ErrorKind.BadResource:
      return BadResource;
    case ErrorKind.Http:
      return Http;
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
  UnexpectedEof: UnexpectedEof,
  BadResource: BadResource,
  Http: Http,
};
