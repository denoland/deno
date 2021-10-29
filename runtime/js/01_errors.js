// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { Error } = window.__bootstrap.primordials;
  const { BadResource, Interrupted } = core;

  class NotFound extends Error {
    constructor(msg) {
      super(msg);
      this.name = "NotFound";
      this.code = "ENOENT";
    }
  }

  class PermissionDenied extends Error {
    constructor(msg) {
      super(msg);
      this.name = "PermissionDenied";
      this.code = "EACCES";
    }
  }

  class ConnectionRefused extends Error {
    constructor(msg) {
      super(msg);
      this.name = "ConnectionRefused";
      this.code = "ECONNREFUSED";
    }
  }

  class ConnectionReset extends Error {
    constructor(msg) {
      super(msg);
      this.name = "ConnectionReset";
      this.code = "ECONNRESET";
    }
  }

  class ConnectionAborted extends Error {
    constructor(msg) {
      super(msg);
      this.name = "ConnectionAborted";
      this.code = "ECONNABORTED";
    }
  }

  class NotConnected extends Error {
    constructor(msg) {
      super(msg);
      this.name = "NotConnected";
      this.code = "ENOTCONN";
    }
  }

  class AddrInUse extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AddrInUse";
      this.code = "EADDRINUSE";
    }
  }

  class AddrNotAvailable extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AddrNotAvailable";
      this.code = "EADDRNOTAVAIL";
    }
  }

  class BrokenPipe extends Error {
    constructor(msg) {
      super(msg);
      this.name = "BrokenPipe";
      this.code = "EPIPE";
    }
  }

  class AlreadyExists extends Error {
    constructor(msg) {
      super(msg);
      this.name = "AlreadyExists";
      this.code = "EEXIST";
    }
  }

  class InvalidData extends Error {
    constructor(msg) {
      super(msg);
      this.name = "InvalidData";
      this.code = "EINVAL";
    }
  }

  class TimedOut extends Error {
    constructor(msg) {
      super(msg);
      this.name = "TimedOut";
      this.code = "ETIMEDOUT";
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
      this.code = "EBUSY";
    }
  }

  class NotSupported extends Error {
    constructor(msg) {
      super(msg);
      this.name = "NotSupported";
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
    NotSupported,
  };

  window.__bootstrap.errors = {
    errors,
  };
})(this);
