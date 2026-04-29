// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// Mirrors Node.js lib/internal/fs/sync_write_stream.js
//
// SyncWriteStream is used for process.stdout/stderr when they are
// backed by regular files (e.g. output redirected to a file). It
// performs synchronous writes via fs.writeSync, matching Node.js
// behavior for FILE-type stdio descriptors.

import { core, primordials } from "ext:core/mod.js";
const {
  FunctionPrototypeCall,
  ObjectSetPrototypeOf,
} = primordials;

// Lazy-load node:stream and node:fs to avoid circular deps during snapshot.
const lazyStream = core.createLazyLoader("node:stream");
const lazyFs = core.createLazyLoader("node:fs");

let _initialized = false;

function SyncWriteStream(fd, options) {
  if (!_initialized) {
    _initialize();
  }
  FunctionPrototypeCall(lazyStream().Writable, this, { autoDestroy: true });

  options = options || {};

  this.fd = fd;
  this.readable = false;
  this.autoClose = options.autoClose === undefined ? true : options.autoClose;
}

function _initialize() {
  const { Writable } = lazyStream();
  ObjectSetPrototypeOf(SyncWriteStream.prototype, Writable.prototype);
  ObjectSetPrototypeOf(SyncWriteStream, Writable);
  _initialized = true;
}

SyncWriteStream.prototype._write = function (chunk, _encoding, cb) {
  try {
    lazyFs().writeSync(this.fd, chunk, 0, chunk.length);
  } catch (e) {
    cb(e);
    return;
  }
  cb();
};

SyncWriteStream.prototype._destroy = function (err, cb) {
  if (this.fd === null) {
    return cb(err);
  }

  if (this.autoClose) {
    lazyFs().closeSync(this.fd);
  }

  this.fd = null;
  cb(err);
};

SyncWriteStream.prototype.destroySoon = SyncWriteStream.prototype.destroy;

export default SyncWriteStream;
