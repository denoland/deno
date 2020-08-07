// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  // Warning! The values in this enum are duplicated in cli/op_error.rs
  // Update carefully!
  const ErrorKind = {
    1: "NotFound",
    2: "PermissionDenied",
    3: "ConnectionRefused",
    4: "ConnectionReset",
    5: "ConnectionAborted",
    6: "NotConnected",
    7: "AddrInUse",
    8: "AddrNotAvailable",
    9: "BrokenPipe",
    10: "AlreadyExists",
    13: "InvalidData",
    14: "TimedOut",
    15: "Interrupted",
    16: "WriteZero",
    17: "UnexpectedEof",
    18: "BadResource",
    19: "Http",
    20: "URIError",
    21: "TypeError",
    22: "Other",
    23: "Busy",

    NotFound: 1,
    PermissionDenied: 2,
    ConnectionRefused: 3,
    ConnectionReset: 4,
    ConnectionAborted: 5,
    NotConnected: 6,
    AddrInUse: 7,
    AddrNotAvailable: 8,
    BrokenPipe: 9,
    AlreadyExists: 10,
    InvalidData: 13,
    TimedOut: 14,
    Interrupted: 15,
    WriteZero: 16,
    UnexpectedEof: 17,
    BadResource: 18,
    Http: 19,
    URIError: 20,
    TypeError: 21,
    Other: 22,
    Busy: 23,
  };

  function getErrorClass(kind) {
    switch (kind) {
      case ErrorKind.TypeError:
      case "TypeError":
        return TypeError;
      case ErrorKind.Other:
      case "Other":
        return Error;
      case ErrorKind.URIError:
      case "URIError":
        return URIError;
      case ErrorKind.NotFound:
      case "NotFound":
        return NotFound;
      case ErrorKind.PermissionDenied:
      case "PermissionDenied":
        return PermissionDenied;
      case ErrorKind.ConnectionRefused:
      case "ConnectionRefused":
        return ConnectionRefused;
      case ErrorKind.ConnectionReset:
      case "ConnectionReset":
        return ConnectionReset;
      case ErrorKind.ConnectionAborted:
      case "ConnectionAborted":
        return ConnectionAborted;
      case ErrorKind.NotConnected:
      case "NotConnected":
        return NotConnected;
      case ErrorKind.AddrInUse:
      case "AddrInUse":
        return AddrInUse;
      case ErrorKind.AddrNotAvailable:
      case "AddrNotAvailable":
        return AddrNotAvailable;
      case ErrorKind.BrokenPipe:
      case "BrokenPipe":
        return BrokenPipe;
      case ErrorKind.AlreadyExists:
      case "AlreadyExists":
        return AlreadyExists;
      case ErrorKind.InvalidData:
      case "InvalidData":
        return InvalidData;
      case ErrorKind.TimedOut:
      case "TimedOut":
        return TimedOut;
      case ErrorKind.Interrupted:
      case "Interrupted":
        return Interrupted;
      case ErrorKind.WriteZero:
      case "WriteZero":
        return WriteZero;
      case ErrorKind.UnexpectedEof:
      case "UnexpectedEof":
        return UnexpectedEof;
      case ErrorKind.BadResource:
      case "BadResource":
        return BadResource;
      case ErrorKind.Http:
      case "Http":
        return Http;
      case ErrorKind.Busy:
      case "Busy":
        return Busy;
    }
  }

  class NotFound extends Error {
    constructor(msg) {
      super(msg);
      this.name = "NotFound";
    }
  }

  class PermissionDenied extends Error {
    constructor(msg) {
      super(msg);
      this.name = "PermissionDenied";
    }
  }

  class ConnectionRefused extends Error {
    constructor(msg) {
      super(msg);
      this.name = "ConnectionRefused";
    }
  }

  class ConnectionReset extends Error {
    constructor(msg) {
      super(msg);
      this.name = "ConnectionReset";
    }
  }

  class ConnectionAborted extends Error {
    constructor(msg) {
      super(msg);
      this.name = "ConnectionAborted";
    }
  }

  class NotConnected extends Error {
    constructor(msg) {
      super(msg);
      this.name = "NotConnected";
    }
  }

  class AddrInUse extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AddrInUse";
    }
  }

  class AddrNotAvailable extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AddrNotAvailable";
    }
  }

  class BrokenPipe extends Error {
    constructor(msg) {
      super(msg);
      this.name = "BrokenPipe";
    }
  }

  class AlreadyExists extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AlreadyExists";
    }
  }

  class InvalidData extends Error {
    constructor(msg) {
      super(msg);
      this.name = "InvalidData";
    }
  }

  class TimedOut extends Error {
    constructor(msg) {
      super(msg);
      this.name = "TimedOut";
    }
  }

  class Interrupted extends Error {
    constructor(msg) {
      super(msg);
      this.name = "Interrupted";
    }
  }

  class WriteZero extends Error {
    constructor(msg) {
      super(msg);
      this.name = "WriteZero";
    }
  }

  class UnexpectedEof extends Error {
    constructor(msg) {
      super(msg);
      this.name = "UnexpectedEof";
    }
  }

  class BadResource extends Error {
    constructor(msg) {
      super(msg);
      this.name = "BadResource";
    }
  }

  class Http extends Error {
    constructor(msg) {
      super(msg);
      this.name = "Http";
    }
  }

  class Busy extends Error {
    constructor(msg) {
      super(msg);
      this.name = "Busy";
    }
  }

  const errors = {
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
    Http,
    Busy,
  };

  window.__bootstrap.errors = {
    errors,
    getErrorClass,
  };
})(this);
