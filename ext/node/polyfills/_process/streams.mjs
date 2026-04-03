// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// Mirrors Node.js lib/internal/bootstrap/switches/is_main_thread.js
// for process.stdin, process.stdout, process.stderr creation.
//
// Streams are created lazily (on first access) to avoid circular
// dependencies between process, node:net, and node:fs during bootstrap.

import { core, primordials } from "ext:core/mod.js";
const { ObjectDefineProperty } = primordials;

import { nextTick } from "ext:deno_node/_next_tick.ts";
import { Readable, Writable } from "node:stream";
import { guessHandleType } from "ext:deno_node/internal_binding/util.ts";
import { op_node_fs_write_sync, op_node_register_fd } from "ext:core/ops";

// Lazy loaders to break circular deps (process -> streams -> net/fs/tty).
const lazyNet = core.createLazyLoader("node:net");
const lazyFs = core.createLazyLoader("node:fs");
const lazyTty = core.createLazyLoader("node:tty");

// Matches Node.js dummyDestroy: prevents the fd from actually being closed.
function dummyDestroy(err, cb) {
  cb(err);
  this._undestroy();
  if (!this._writableState.emitClose) {
    nextTick(() => this.emit("close"));
  }
}

// Synchronous write stream backed by raw fd ops.
// Used for FILE-type stdout/stderr and as fallback for PIPE/TCP
// when the module system isn't ready yet.
function _createSyncWriteStream(fd) {
  return new Writable({
    autoDestroy: true,
    emitClose: false,
    write(chunk, _encoding, cb) {
      try {
        op_node_fs_write_sync(fd, chunk, -1);
      } catch (e) {
        cb(e);
        return;
      }
      cb();
    },
  });
}

// https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js
function createWritableStdioStream(fd) {
  let stream;
  const type = guessHandleType(fd);

  switch (type) {
    case "TTY":
      stream = new (lazyTty().WriteStream)(fd);
      stream._type = "tty";
      break;
    case "FILE":
      // Register the fd so fd-based ops work. On Unix this dups the fd
      // internally to avoid double-ownership with the resource table.
      op_node_register_fd(fd);
      stream = _createSyncWriteStream(fd);
      stream._type = "fs";
      break;
    case "PIPE":
    case "TCP": {
      const { Socket } = lazyNet();
      stream = new Socket({
        fd,
        readable: false,
        writable: true,
      });
      stream._type = "pipe";
      break;
    }
    default:
      // For non-console Windows apps or unknown handle types.
      stream = new Writable({
        write(_buf, _enc, cb) {
          cb();
        },
      });
  }

  stream.fd = fd;
  stream._isStdio = true;
  stream.destroySoon = stream.destroy;
  stream._destroy = dummyDestroy;

  // Stdio sockets must not prevent the process from exiting.
  if (stream._handle && stream._handle.unref) {
    stream._handle.unref();
  }

  return stream;
}

// https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js
function createStdin() {
  const fd = 0;
  let stdin;
  const type = guessHandleType(fd);

  switch (type) {
    case "TTY":
      stdin = new (lazyTty().ReadStream)(fd);
      break;
    case "FILE": {
      const { ReadStream } = lazyFs();
      stdin = new ReadStream(null, { fd, autoClose: false });
      break;
    }
    case "PIPE":
    case "TCP": {
      const { Socket } = lazyNet();
      stdin = new Socket({
        fd,
        readable: true,
        writable: false,
        manualStart: true,
      });
      // Make sure the stdin can't be .end()-ed
      stdin._writableState.ended = true;
      break;
    }
    default: {
      // Provide a dummy contentless input for e.g. non-console
      // Windows applications.
      stdin = new Readable({ read() {} });
      // deno-lint-ignore prefer-primordials
      stdin.push(null);
    }
  }

  stdin.fd = fd;

  // stdin starts out life in a paused state. Explicitly readStop() it
  // to put it in the not-reading state.
  if (stdin._handle?.readStop) {
    stdin._handle.reading = false;
    stdin._readableState.reading = false;
    stdin._handle.readStop();
  }

  // Stdin must not prevent the process from exiting when idle.
  if (stdin._handle?.unref) {
    stdin._handle.unref();
  }

  function onpause() {
    if (!stdin._handle) {
      return;
    }
    if (stdin._handle.reading && !stdin.readableFlowing) {
      stdin._readableState.reading = false;
      stdin._handle.reading = false;
      stdin._handle.readStop();
    }
  }

  stdin.on("pause", () => nextTick(onpause));

  return stdin;
}

// Warmup-safe writable for snapshot: uses core.writeSync directly
// (works during snapshot since resources 0/1/2 exist at that point).
function createWarmupWritable(fd) {
  const stream = new Writable({
    emitClose: false,
    write(buf, _enc, cb) {
      try {
        Deno[Deno.internal].core.writeSync(fd, buf);
      } catch {
        // ignore errors during warmup
      }
      cb();
    },
  });
  stream.fd = fd;
  stream._isStdio = true;
  stream.destroySoon = stream.destroy;
  stream._destroy = dummyDestroy;
  return stream;
}

/**
 * Define lazy stdio getters on the process object, matching Node.js's
 * is_main_thread.js defineStream pattern. Streams are created on first
 * access, by which time all modules (net, fs, tty) are fully loaded.
 */
export function setupStdio(process, warmup = false) {
  if (warmup) {
    // During snapshot, create simple warmup streams immediately.
    // They'll be replaced at boot time via setupStdio(process).
    process.stdin = null; // TTY handle can't be created in snapshot
    process.stdout = createWarmupWritable(1);
    process.stderr = createWarmupWritable(2);
    return;
  }

  let stdin, stdout, stderr;

  ObjectDefineProperty(process, "stdout", {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      if (!stdout) stdout = createWritableStdioStream(1);
      return stdout;
    },
    set(val) {
      stdout = val;
    },
  });

  ObjectDefineProperty(process, "stderr", {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      if (!stderr) stderr = createWritableStdioStream(2);
      return stderr;
    },
    set(val) {
      stderr = val;
    },
  });

  ObjectDefineProperty(process, "stdin", {
    __proto__: null,
    configurable: true,
    enumerable: true,
    get() {
      if (!stdin) stdin = createStdin();
      return stdin;
    },
    set(val) {
      stdin = val;
    },
  });
}
