// Copyright 2018-2025 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
const { BadResource, Interrupted, NotCapable } = core;
const { Error } = primordials;

class NotFound extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "NotFound";
  }
}

class ConnectionRefused extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "ConnectionRefused";
  }
}

class ConnectionReset extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "ConnectionReset";
  }
}

class ConnectionAborted extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "ConnectionAborted";
  }
}

class NotConnected extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "NotConnected";
  }
}

class AddrInUse extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "AddrInUse";
  }
}

class AddrNotAvailable extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "AddrNotAvailable";
  }
}

class BrokenPipe extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "BrokenPipe";
  }
}

class AlreadyExists extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "AlreadyExists";
  }
}

class InvalidData extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "InvalidData";
  }
}

class TimedOut extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "TimedOut";
  }
}

class WriteZero extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "WriteZero";
  }
}

class WouldBlock extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "WouldBlock";
  }
}

class UnexpectedEof extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "UnexpectedEof";
  }
}

class Http extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "Http";
  }
}

class Busy extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "Busy";
  }
}

class PermissionDenied extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "PermissionDenied";
  }
}

class NotSupported extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "NotSupported";
  }
}

class FilesystemLoop extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "FilesystemLoop";
  }
}

class IsADirectory extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "IsADirectory";
  }
}

class NetworkUnreachable extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "NetworkUnreachable";
  }
}

class NotADirectory extends Error {
  constructor(msg, opts) {
    super(msg, opts);
    this.name = "NotADirectory";
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
  WouldBlock,
  UnexpectedEof,
  BadResource,
  Http,
  Busy,
  NotSupported,
  FilesystemLoop,
  IsADirectory,
  NetworkUnreachable,
  NotADirectory,
  NotCapable,
};

export { errors };
