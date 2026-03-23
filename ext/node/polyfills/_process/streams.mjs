// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  Uint8ArrayPrototype,
  Error,
  ObjectDefineProperties,
  ObjectDefineProperty,
  TypedArrayPrototypeSlice,
  PromisePrototypeThen,
  ObjectValues,
  ObjectPrototypeIsPrototypeOf,
} = primordials;

import { Buffer } from "node:buffer";
import {
  clearLine,
  clearScreenDown,
  cursorTo,
  moveCursor,
} from "ext:deno_node/internal/readline/callbacks.mjs";
import { nextTick } from "ext:deno_node/_next_tick.ts";
import { Duplex, Readable, Writable } from "node:stream";
import * as io from "ext:deno_io/12_io.js";
import { guessHandleType } from "ext:deno_node/internal_binding/util.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import { op_bootstrap_color_depth } from "ext:core/ops";
import { validateInteger } from "ext:deno_node/internal/validators.mjs";
import { WriteStream as TTYWriteStream } from "ext:deno_node/internal/tty.js";

// Matches Node.js dummyDestroy in is_main_thread.js -- stdio streams must not
// actually close so that libraries (e.g. mute-stream / @inquirer/prompts) that
// call destroy()/end() on process.stdout between prompts keep working.
function dummyDestroy(err, cb) {
  cb(err);
  this._undestroy();
  if (!this._writableState.emitClose) {
    nextTick(() => this.emit("close"));
  }
}

// Mirrors Node.js createWritableStdioStream in is_main_thread.js.
// Detects handle type and creates the appropriate stream:
//   TTY   -> tty.WriteStream (backed by libuv uv_tty_t handle)
//   other -> plain Writable using Deno IO writeSync
// https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js
export function createWritableStdioStream(fd, warmup = false) {
  const handleType = warmup ? "TTY" : guessHandleType(fd);
  let stream;

  if (handleType === "TTY" && !warmup) {
    // TTY: use a proper tty.WriteStream backed by a libuv handle, matching
    // Node.js exactly. All TTY features (columns, rows, getColorDepth,
    // cursor methods, SIGWINCH) are provided natively by WriteStream.
    stream = new TTYWriteStream(fd);
    // Override _destroy so the fd is never actually closed -- matches Node.js
    // dummyDestroy in is_main_thread.js.
    stream._destroy = dummyDestroy;
  } else {
    // Non-TTY (PIPE, FILE, etc.) or warmup: use a Writable that writes
    // through Deno IO. Once net.Socket supports creation from a raw fd
    // (new Socket({ fd })), non-TTY should switch to net.Socket for PIPE/TCP
    // and SyncWriteStream for FILE to match Node.js.
    const writer = fd === 1 ? io.stdout : fd === 2 ? io.stderr : null;
    const name = fd === 1 ? "stdout" : "stderr";

    stream = new Writable({
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
      destroy: dummyDestroy,
    });

    // For non-TTY streams, add isTTY getter (returns false) so code that
    // checks `process.stdout.isTTY` works. Also add getColorDepth/hasColors
    // for libraries that call these unconditionally.
    let getIsTTY = () => writer?.isTerminal();
    ObjectDefineProperties(stream, {
      isTTY: {
        __proto__: null,
        enumerable: true,
        configurable: true,
        get: () => getIsTTY(),
        set: (value) => {
          getIsTTY = () => value;
        },
      },
      getColorDepth: {
        __proto__: null,
        enumerable: true,
        configurable: true,
        writable: true,
        value: () => op_bootstrap_color_depth(),
      },
      hasColors: {
        __proto__: null,
        enumerable: true,
        configurable: true,
        writable: true,
        value: (count, env) => {
          if (
            env === undefined &&
            (count === undefined ||
              typeof count === "object" && count !== null)
          ) {
            env = count;
            count = 16;
          } else {
            validateInteger(count, "count", 2);
          }
          const depth = op_bootstrap_color_depth();
          return count <= 2 ** depth;
        },
      },
    });

    // Warmup: add TTY-like methods so snapshot-time code works.
    // Replaced at boot with a proper tty.WriteStream (TTY) or fresh Writable.
    if (warmup) {
      // During warmup, also add columns/rows/getWindowSize that the real
      // TTY WriteStream would provide.
      const getColumns = () =>
        stream._columns ||
        (writer?.isTerminal() ? Deno.consoleSize?.().columns : undefined);
      ObjectDefineProperties(stream, {
        columns: {
          __proto__: null,
          enumerable: true,
          configurable: true,
          get: () => getColumns(),
          set: (value) => {
            stream._columns = value;
          },
        },
        rows: {
          __proto__: null,
          enumerable: true,
          configurable: true,
          get: () =>
            writer?.isTerminal() ? Deno.consoleSize?.().rows : undefined,
        },
        getWindowSize: {
          __proto__: null,
          enumerable: true,
          configurable: true,
          value: () =>
            writer?.isTerminal()
              ? ObjectValues(Deno.consoleSize?.())
              : undefined,
        },
      });

      stream.cursorTo = function (x, y, callback) {
        return cursorTo(this, x, y, callback);
      };
      stream.moveCursor = function (dx, dy, callback) {
        return moveCursor(this, dx, dy, callback);
      };
      stream.clearLine = function (dir, callback) {
        return clearLine(this, dir, callback);
      };
      stream.clearScreenDown = function (callback) {
        return clearScreenDown(this, callback);
      };
    }
  }

  stream.fd = fd;
  stream.destroySoon = stream.destroy;
  stream._isStdio = true;

  return stream;
}

function _guessStdinType(fd) {
  if (typeof fd !== "number" || fd < 0) return "UNKNOWN";
  return guessHandleType(fd);
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

let readStream;
export function setReadStream(s) {
  readStream = s;
}

/** https://nodejs.org/api/process.html#process_process_stdin */
// https://github.com/nodejs/node/blob/v18.12.1/lib/internal/bootstrap/switches/is_main_thread.js#L189
/** Create process.stdin */
export const initStdin = (warmup = false) => {
  const fd = io.stdin ? io.STDIN_RID : undefined;
  let stdin;
  // Warmup assumes a TTY for all stdio
  const stdinType = warmup ? "TTY" : _guessStdinType(fd);

  switch (stdinType) {
    case "FILE": {
      // Since `fs.ReadStream` cannot be imported before process initialization,
      // use `Readable` instead.
      // https://github.com/nodejs/node/blob/v18.12.1/lib/internal/bootstrap/switches/is_main_thread.js#L200
      // https://github.com/nodejs/node/blob/v18.12.1/lib/internal/fs/streams.js#L148
      stdin = new Readable({
        highWaterMark: 64 * 1024,
        autoDestroy: false,
        read: _read,
      });
      break;
    }
    case "TTY": {
      // FIXME: We should be able to create stdin handle during warmup and re-use it but
      // cppgc object wraps crash in snapshot mode.
      //
      // To reproduce crash, change the condition to `if (!warmup)` below:
      if (warmup) {
        return null;
      }
      // TTY stdin is a proper tty.ReadStream backed by a libuv uv_tty_t handle.
      // It already has setRawMode (via uv_tty_set_mode), isRaw, isTTY, and a
      // proper _handle with readStart/readStop -- do NOT override these with
      // Deno IO layer equivalents (io.stdin.setRaw / io.stdin.isTerminal),
      // as that conflates two different raw mode mechanisms (libuv vs op_set_raw).
      stdin = new readStream(fd);
      stdin.fd = io.stdin ? io.STDIN_RID : -1;

      // `stdin` starts out life in a paused state. Explicitly readStop() it
      // to put it in the not-reading state.
      // Ref: https://github.com/nodejs/node/blob/main/lib/internal/bootstrap/switches/is_main_thread.js
      if (stdin._handle?.readStop) {
        stdin._handle.reading = false;
        stdin._readableState.reading = false;
        stdin._handle.readStop();
      }

      // If the user calls stdin.pause(), then we need to stop reading
      // once the stream implementation does so (one nextTick later),
      // so that the process can close down.
      stdin.on("pause", () => {
        nextTick(() => {
          if (!stdin._handle) return;
          if (stdin._handle.reading && !stdin.readableFlowing) {
            stdin._readableState.reading = false;
            stdin._handle.reading = false;
            stdin._handle.readStop();
          }
        });
      });

      return stdin;
    }
    case "PIPE":
    case "TCP": {
      // For PIPE and TCP, `new Duplex()` should be replaced `new net.Socket()` if possible.
      // There are two problems that need to be resolved.
      // 1. Using them here introduces a circular dependency.
      // 2. Creating a net.Socket() from a fd is not currently supported.
      // https://github.com/nodejs/node/blob/v18.12.1/lib/internal/bootstrap/switches/is_main_thread.js#L206
      // https://github.com/nodejs/node/blob/v18.12.1/lib/net.js#L329
      stdin = new Duplex({
        readable: true,
        writable: false,
        allowHalfOpen: false,
        emitClose: false,
        autoDestroy: true,
        decodeStrings: false,
        read: _read,
      });

      // Make sure the stdin can't be `.end()`-ed
      stdin._writableState.ended = true;

      // Provide a minimal _handle so code that checks process.stdin._handle
      // (e.g. test-stdout-close-unref.js) works. We intentionally omit
      // readStart/readStop/reading so the onpause handler takes the simple
      // io.stdin UNREF path - adding those methods causes _readableState.reading
      // to be reset, which triggers duplicate _read() calls and orphaned
      // reffed promises that prevent process exit.
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
    }
    default: {
      // Provide a dummy contentless input for e.g. non-console
      // Windows applications.
      stdin = new Readable({ read() {} });
      // deno-lint-ignore prefer-primordials
      stdin.push(null);
    }
  }

  // Common setup for non-TTY stdin types (FILE, PIPE, TCP, default).
  // TTY stdin returns early above -- it uses the ReadStream's own libuv handle
  // and does not need these Deno IO layer shims.

  stdin.on("close", () => io.stdin?.close());
  stdin.fd = io.stdin ? io.STDIN_RID : -1;

  // `stdin` starts out life in a paused state. Explicitly to readStop() it to put it in the
  // not-reading state.
  if (stdin._handle?.readStop) {
    stdin._handle.reading = false;
    stdin._readableState.reading = false;
    stdin._handle.readStop();
  }

  function onpause() {
    if (!stdin._handle || !stdin._handle.readStop) {
      // This allows the process to exit when stdin is paused.
      io.stdin?.[io.UNREF]();
      return;
    }

    if (stdin._handle.reading && !stdin.readableFlowing) {
      stdin._readableState.reading = false;
      stdin._handle.reading = false;
      stdin._handle.readStop();
    }
  }

  // If the user calls stdin.pause(), then we need to stop reading
  // once the stream implementation does so (one nextTick later),
  // so that the process can close down.
  stdin.on("pause", () => nextTick(onpause));

  // For non-TTY streams, provide isTTY/setRawMode/isRaw shims via Deno IO.
  // (TTY ReadStream has these natively via its libuv handle.)
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
};
