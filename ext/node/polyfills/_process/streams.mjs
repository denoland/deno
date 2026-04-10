// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// This module creates process.stdin/stdout/stderr following the exact same
// pattern as Node.js's lib/internal/bootstrap/switches/is_main_thread.js.
//
// The key insight: stdio streams are created based on guessHandleType(fd):
//   TTY       -> tty.WriteStream / tty.ReadStream
//   PIPE/TCP  -> net.Socket({ fd })
//   FILE      -> SyncWriteStream (stdout/stderr) / fs.ReadStream (stdin)
//   UNKNOWN   -> dummy stream

import { core, primordials } from "ext:core/mod.js";
const {
  ObjectDefineProperty,
  Readable,
  Writable,
} = primordials;

import { nextTick } from "ext:deno_node/_next_tick.ts";
import { guessHandleType } from "ext:deno_node/internal_binding/util.ts";
import SyncWriteStream from "ext:deno_node/internal/fs/sync_write_stream.js";

// Lazy loaders to avoid circular dependencies during bootstrap.
// tty, net, and fs all depend on process, so we can't import them eagerly.
const lazyNet = core.createLazyLoader("node:net");
const lazyFs = core.createLazyLoader("node:fs");

let readStream;
export function setReadStream(s) {
  readStream = s;
}

let writeStream;
export function setWriteStream(s) {
  writeStream = s;
}

// The no-op destroy that Node.js uses to make stdout/stderr indestructible.
// Ref: https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js
function dummyDestroy(err, cb) {
  cb(err);
  this._undestroy();

  if (!this._writableState.emitClose) {
    nextTick(() => this.emit("close"));
  }
}

/**
 * Create process.stdout or process.stderr, matching Node.js exactly.
 * Ref: https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js#L41
 */
export function createWritableStdioStream(fd) {
  const handleType = guessHandleType(fd);
  let stream;

  switch (handleType) {
    case "TTY": {
      stream = new writeStream(fd);
      break;
    }
    case "FILE": {
      stream = new SyncWriteStream(fd, { autoClose: false });
      stream._type = "fs";
      break;
    }
    case "PIPE":
    case "TCP": {
      const net = lazyNet();
      stream = new net.Socket({
        fd,
        readable: false,
        writable: true,
        manualStart: true,
      });
      stream._type = "pipe";
      break;
    }
    default: {
      // UNKNOWN: dummy writable that discards everything (e.g. non-console
      // Windows applications).
      const { Writable: StreamWritable } = core.createLazyLoader(
        "node:stream",
      )();
      stream = new StreamWritable({
        write(_buf, _enc, cb) {
          cb();
        },
      });
    }
  }

  stream.fd = fd;
  stream._isStdio = true;
  stream.destroySoon = stream.destroy;

  // Make stdout/stderr indestructible (match Node.js behavior).
  // Libraries like mute-stream call destroy()/end() on process.stdout
  // between prompts. Without this, the underlying handle is closed.
  stream._destroy = dummyDestroy;

  return stream;
}

function _guessStdinType(fd) {
  if (typeof fd !== "number" || fd < 0) return "UNKNOWN";
  return guessHandleType(fd);
}

/**
 * Create process.stdin, matching Node.js exactly.
 * Ref: https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js#L166
 */
export function createStdin(fd) {
  const stdinType = _guessStdinType(fd);
  let stdin;

  switch (stdinType) {
    case "TTY": {
      stdin = new readStream(fd);
      break;
    }
    case "FILE": {
      const fs = lazyFs();
      stdin = new fs.ReadStream(null, { fd, autoClose: false });
      break;
    }
    case "PIPE":
    case "TCP": {
      const net = lazyNet();
      stdin = new net.Socket({
        fd,
        readable: true,
        writable: false,
        manualStart: true,
      });
      // Make sure the stdin can't be `.end()`-ed
      stdin._writableState.ended = true;
      break;
    }
    default: {
      // UNKNOWN: dummy readable that immediately pushes EOF.
      const { Readable: StreamReadable } = core.createLazyLoader(
        "node:stream",
      )();
      stdin = new StreamReadable({ read() {} });
      // deno-lint-ignore prefer-primordials
      stdin.push(null);
    }
  }

  stdin.fd = fd;

  // stdin starts paused. For handle-based streams (TTY, PIPE/TCP),
  // explicitly stop reading so the process can exit if nothing reads stdin.
  // Ref: https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js#L208
  if (stdin._handle?.readStop) {
    stdin._handle.reading = false;
    stdin._readableState.reading = false;
    stdin._handle.readStop();
  }

  // If the user calls stdin.pause(), stop reading so the process can exit.
  // Ref: https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js#L216
  function onpause() {
    if (!stdin._handle) {
      return;
    }

    if (stdin._handle.reading && !stdin.readableFlowing) {
      stdin._readableState.reading = false;
      stdin._handle.reading = false;
      if (stdin._handle.readStop) {
        stdin._handle.readStop();
      }
    }
  }

  stdin.on("pause", () => nextTick(onpause));

  return stdin;
}

// Warmup streams for snapshot. These are placeholders created during snapshot
// that get replaced at actual boot time.
export function createWarmupStdout() {
  return _createWarmupWritable(1);
}

export function createWarmupStderr() {
  return _createWarmupWritable(2);
}

export function createWarmupStdin() {
  // FIXME: We should be able to create stdin handle during warmup and re-use it
  // but cppgc object wraps crash in snapshot mode.
  return null;
}

function _createWarmupWritable(fd) {
  const { Writable: StreamWritable } = core.createLazyLoader("node:stream")();
  const stream = new StreamWritable({
    emitClose: false,
    write(_buf, _enc, cb) {
      cb();
    },
    destroy(err, cb) {
      cb(err);
      this._undestroy();
      if (!this._writableState.emitClose) {
        nextTick(() => this.emit("close"));
      }
    },
  });
  stream.fd = fd;
  stream._isStdio = true;
  stream.destroySoon = stream.destroy;
  stream.isTTY = true; // assume TTY during warmup
  return stream;
}
