// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/errors.ts", [], function (exports_9, context_9) {
  "use strict";
  let ErrorKind,
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    BrokenPipe,
    AlreadyExists,
    InvalidData,
    TimedOut,
    Interrupted,
    WriteZero,
    UnexpectedEof,
    BadResource,
    Http;
  const __moduleName = context_9 && context_9.id;
  function getErrorClass(kind) {
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
  exports_9("getErrorClass", getErrorClass);
  return {
    setters: [],
    execute: function () {
      // Warning! The values in this enum are duplicated in cli/op_error.rs
      // Update carefully!
      (function (ErrorKind) {
        ErrorKind[(ErrorKind["NotFound"] = 1)] = "NotFound";
        ErrorKind[(ErrorKind["PermissionDenied"] = 2)] = "PermissionDenied";
        ErrorKind[(ErrorKind["ConnectionRefused"] = 3)] = "ConnectionRefused";
        ErrorKind[(ErrorKind["ConnectionReset"] = 4)] = "ConnectionReset";
        ErrorKind[(ErrorKind["ConnectionAborted"] = 5)] = "ConnectionAborted";
        ErrorKind[(ErrorKind["NotConnected"] = 6)] = "NotConnected";
        ErrorKind[(ErrorKind["AddrInUse"] = 7)] = "AddrInUse";
        ErrorKind[(ErrorKind["AddrNotAvailable"] = 8)] = "AddrNotAvailable";
        ErrorKind[(ErrorKind["BrokenPipe"] = 9)] = "BrokenPipe";
        ErrorKind[(ErrorKind["AlreadyExists"] = 10)] = "AlreadyExists";
        ErrorKind[(ErrorKind["InvalidData"] = 13)] = "InvalidData";
        ErrorKind[(ErrorKind["TimedOut"] = 14)] = "TimedOut";
        ErrorKind[(ErrorKind["Interrupted"] = 15)] = "Interrupted";
        ErrorKind[(ErrorKind["WriteZero"] = 16)] = "WriteZero";
        ErrorKind[(ErrorKind["UnexpectedEof"] = 17)] = "UnexpectedEof";
        ErrorKind[(ErrorKind["BadResource"] = 18)] = "BadResource";
        ErrorKind[(ErrorKind["Http"] = 19)] = "Http";
        ErrorKind[(ErrorKind["URIError"] = 20)] = "URIError";
        ErrorKind[(ErrorKind["TypeError"] = 21)] = "TypeError";
        ErrorKind[(ErrorKind["Other"] = 22)] = "Other";
      })(ErrorKind || (ErrorKind = {}));
      exports_9("ErrorKind", ErrorKind);
      NotFound = class NotFound extends Error {
        constructor(msg) {
          super(msg);
          this.name = "NotFound";
        }
      };
      PermissionDenied = class PermissionDenied extends Error {
        constructor(msg) {
          super(msg);
          this.name = "PermissionDenied";
        }
      };
      ConnectionRefused = class ConnectionRefused extends Error {
        constructor(msg) {
          super(msg);
          this.name = "ConnectionRefused";
        }
      };
      ConnectionReset = class ConnectionReset extends Error {
        constructor(msg) {
          super(msg);
          this.name = "ConnectionReset";
        }
      };
      ConnectionAborted = class ConnectionAborted extends Error {
        constructor(msg) {
          super(msg);
          this.name = "ConnectionAborted";
        }
      };
      NotConnected = class NotConnected extends Error {
        constructor(msg) {
          super(msg);
          this.name = "NotConnected";
        }
      };
      AddrInUse = class AddrInUse extends Error {
        constructor(msg) {
          super(msg);
          this.name = "AddrInUse";
        }
      };
      AddrNotAvailable = class AddrNotAvailable extends Error {
        constructor(msg) {
          super(msg);
          this.name = "AddrNotAvailable";
        }
      };
      BrokenPipe = class BrokenPipe extends Error {
        constructor(msg) {
          super(msg);
          this.name = "BrokenPipe";
        }
      };
      AlreadyExists = class AlreadyExists extends Error {
        constructor(msg) {
          super(msg);
          this.name = "AlreadyExists";
        }
      };
      InvalidData = class InvalidData extends Error {
        constructor(msg) {
          super(msg);
          this.name = "InvalidData";
        }
      };
      TimedOut = class TimedOut extends Error {
        constructor(msg) {
          super(msg);
          this.name = "TimedOut";
        }
      };
      Interrupted = class Interrupted extends Error {
        constructor(msg) {
          super(msg);
          this.name = "Interrupted";
        }
      };
      WriteZero = class WriteZero extends Error {
        constructor(msg) {
          super(msg);
          this.name = "WriteZero";
        }
      };
      UnexpectedEof = class UnexpectedEof extends Error {
        constructor(msg) {
          super(msg);
          this.name = "UnexpectedEof";
        }
      };
      BadResource = class BadResource extends Error {
        constructor(msg) {
          super(msg);
          this.name = "BadResource";
        }
      };
      Http = class Http extends Error {
        constructor(msg) {
          super(msg);
          this.name = "Http";
        }
      };
      exports_9("errors", {
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
      });
    },
  };
});
