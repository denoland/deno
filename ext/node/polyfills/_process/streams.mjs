// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// Mirrors Node.js lib/internal/bootstrap/switches/is_main_thread.js
// for process.stdin, process.stdout, process.stderr creation.
//
// Streams are created lazily (on first access) to avoid circular
// dependencies between process, node:net, and node:fs during bootstrap.

import { core, primordials } from "ext:core/mod.js";
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  TypedArrayPrototypeSlice,
  Uint8ArrayPrototype,
  Error,
  PromisePrototypeThen,
} = primordials;

import { Buffer } from "node:buffer";
import { nextTick } from "ext:deno_node/_next_tick.ts";
import { Duplex, Readable, Writable } from "node:stream";
import * as io from "ext:deno_io/12_io.js";
import { guessHandleType } from "ext:deno_node/internal_binding/util.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";

// Lazy loaders to break circular deps (process -> streams -> net/fs/tty).
const lazyTty = core.createLazyLoader("node:tty");

// Get the Deno io object for a stdio fd (0=stdin, 1=stdout, 2=stderr).
function _getDenoWriter(fd) {
  if (fd === 1) return io.stdout;
  if (fd === 2) return io.stderr;
  return null;
}

// Matches Node.js dummyDestroy: prevents the fd from actually being closed.
function dummyDestroy(err, cb) {
  cb(err);
  this._undestroy();
  if (!this._writableState.emitClose) {
    nextTick(() => this.emit("close"));
  }
}

// Creates a Writable stream backed by a Deno writer (io.stdout / io.stderr).
// This is the primary path for non-TTY stdout/stderr.
function _createWritableFromDenoWriter(writer, name) {
  return new Writable({
    emitClose: false,
    write(buf, enc, cb) {
      if (!writer) {
        this.destroy(
          new Error(`Deno.${name} is not available in this environment`),
        );
        return;
      }
      try {
        let data = ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, buf)
          ? buf
          : Buffer.from(buf, enc);
        // Handle partial writes - writeSync may not write all bytes at once
        // (e.g., when stdout is a pipe and the pipe buffer is near capacity).
        // deno-lint-ignore prefer-primordials
        while (data.byteLength > 0) {
          const nwritten = writer.writeSync(data);
          // deno-lint-ignore prefer-primordials
          if (nwritten >= data.byteLength) break;
          data = TypedArrayPrototypeSlice(data, nwritten);
        }
      } catch (e) {
        if (
          ObjectPrototypeIsPrototypeOf(Deno.errors.BrokenPipe.prototype, e)
        ) {
          const err = new Error("write EPIPE");
          err.code = "EPIPE";
          err.errno = codeMap.get("EPIPE");
          err.syscall = "write";
          cb(err);
          return;
        }
        throw e;
      }
      cb();
    },
  });
}

const _read = function (size) {
  io.stdin?.[io.REF]();
  const p = Buffer.alloc(size || 16 * 1024);
  PromisePrototypeThen(io.stdin?.read(p), (length) => {
    // deno-lint-ignore prefer-primordials
    this.push(length === null ? null : TypedArrayPrototypeSlice(p, 0, length));
  }, (error) => {
    this.destroy(error);
  });
};

// https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js
function createWritableStdioStream(fd) {
  let stream;
  const writer = _getDenoWriter(fd);

  // Try TTY first. On most platforms isTerminal() is reliable. On Git Bash
  // (Windows), isTerminal() may return true for pipe fds; uv_tty_init then
  // fails with EBADF. The try-catch handles that by falling through to the
  // non-TTY path.
  if (writer?.isTerminal()) {
    try {
      stream = new (lazyTty().WriteStream)(fd);
      stream._type = "tty";
    } catch {
      // TTY init failed (e.g. EBADF on Git Bash) - fall through
      stream = null;
    }
  }

  if (!stream) {
    // Non-TTY: use Deno's io writer for stdio fds, falling back to
    // guessHandleType for non-stdio fds (future use).
    if (writer) {
      stream = _createWritableFromDenoWriter(
        writer,
        fd === 1 ? "stdout" : "stderr",
      );
    } else {
      stream = new Writable({
        write(_buf, _enc, cb) {
          cb();
        },
      });
    }
  }

  // deno-lint-ignore prefer-primordials
  stream.fd = writer instanceof io.Stdout
    ? io.STDOUT_RID
    // deno-lint-ignore prefer-primordials
    : writer instanceof io.Stderr
    ? io.STDERR_RID
    : -1;
  stream._isStdio = true;
  stream.destroySoon = stream.destroy;
  stream._destroy = dummyDestroy;

  return stream;
}

// https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js
function createStdin() {
  const fd = 0;
  let stdin;

  // Same TTY-first strategy as createWritableStdioStream.
  if (io.stdin?.isTerminal()) {
    try {
      stdin = new (lazyTty().ReadStream)(fd);
    } catch {
      stdin = null;
    }
  }

  if (!stdin) {
    const stdinType = guessHandleType(fd);
    switch (stdinType) {
      case "TTY":
        stdin = new (lazyTty().ReadStream)(fd);
        break;
      case "PIPE":
      case "TCP":
        stdin = new Duplex({
          readable: true,
          writable: false,
          readableHighWaterMark: undefined,
          allowHalfOpen: false,
          emitClose: false,
          autoDestroy: true,
          decodeStrings: false,
          read: _read,
        });
        stdin._writableState.ended = true;
        stdin._handle = {
          close(cb) {
            io.stdin?.close();
            if (typeof cb === "function") cb();
          },
          ref() {
            io.stdin?.[io.REF]();
          },
          unref() {
            io.stdin?.[io.UNREF]();
          },
          getAsyncId() {
            return -1;
          },
        };
        break;
      default:
        stdin = new Readable({ read() {} });
        // deno-lint-ignore prefer-primordials
        stdin.push(null);
    }
  }

  stdin.on("close", () => io.stdin?.close());
  stdin.fd = io.stdin ? io.STDIN_RID : -1;

  if (stdin._handle?.readStop) {
    stdin._handle.reading = false;
    stdin._readableState.reading = false;
    stdin._handle.readStop();
  }

  function onpause() {
    if (!stdin._handle || !stdin._handle.readStop) {
      io.stdin?.[io.UNREF]();
      return;
    }
    if (stdin._handle.reading && !stdin.readableFlowing) {
      stdin._readableState.reading = false;
      stdin._handle.reading = false;
      stdin._handle.readStop();
    }
  }

  stdin.on("pause", () => nextTick(onpause));

  // Allow users to overwrite isTTY for test isolation and terminal mocking.
  let getStdinIsTTY = () => io.stdin?.isTerminal();
  ObjectDefineProperty(stdin, "isTTY", {
    __proto__: null,
    enumerable: true,
    configurable: true,
    get() {
      return getStdinIsTTY();
    },
    set(value) {
      getStdinIsTTY = () => value;
    },
  });
  stdin._isRawMode = false;
  stdin.setRawMode = (enable) => {
    if (io.stdin?.isTerminal()) {
      io.stdin.setRaw(enable);
    }
    stdin._isRawMode = enable;
    return stdin;
  };
  ObjectDefineProperty(stdin, "isRaw", {
    __proto__: null,
    enumerable: true,
    configurable: true,
    get() {
      return stdin._isRawMode;
    },
  });

  return stdin;
}

// Warmup-safe writable for snapshot: uses core.writeSync directly.
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
    process.stdin = null;
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
