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

// https://github.com/nodejs/node/blob/00738314828074243c9a52a228ab4c68b04259ef/lib/internal/bootstrap/switches/is_main_thread.js#L41
export function createWritableStdioStream(writer, name, warmup = false) {
  const stream = new Writable({
    emitClose: false,
    write(buf, enc, cb) {
      if (!writer) {
        this.destroy(
          new Error(`Deno.${name} is not available in this environment`),
        );
        return;
      }
      // TODO(fraidev): This try/catch is a workaround. When process.stdout
      // is a pipe (not a TTY), Node.js backs it with a real fd-based net.Socket
      // so BrokenPipe flows naturally through stream_wrap.ts as EPIPE. Deno
      // always uses createWritableStdioStream(io.stdout) regardless of pipe/TTY,
      // so BrokenPipe throws synchronously here instead. Once net.Socket supports
      // being created from a raw fd (new Socket({ fd: 1 })), process.stdout/stderr
      // should be switched to net.Socket for non-TTY cases and this can be removed.
      try {
        writer.writeSync(
          ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, buf)
            ? buf
            : Buffer.from(buf, enc),
        );
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
    destroy(err, cb) {
      cb(err);
      this._undestroy();

      // We need to emit 'close' anyway so that the closing
      // of the stream is observable.
      if (!this._writableState.emitClose) {
        nextTick(() => this.emit("close"));
      }
    },
  });
  let fd = -1;

  // deno-lint-ignore prefer-primordials
  if (writer instanceof io.Stdout) {
    fd = io.STDOUT_RID;
    // deno-lint-ignore prefer-primordials
  } else if (writer instanceof io.Stderr) {
    fd = io.STDERR_RID;
  }
  stream.fd = fd;
  stream.destroySoon = stream.destroy;
  stream._isStdio = true;

  // We cannot call `writer?.isTerminal()` eagerly here
  let getIsTTY = () => writer?.isTerminal();
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
      get: () => writer?.isTerminal() ? Deno.consoleSize?.().rows : undefined,
    },
    isTTY: {
      __proto__: null,
      enumerable: true,
      configurable: true,
      // Allow users to overwrite it
      get: () => getIsTTY(),
      set: (value) => {
        getIsTTY = () => value;
      },
    },
    getWindowSize: {
      __proto__: null,
      enumerable: true,
      configurable: true,
      value: () =>
        writer?.isTerminal() ? ObjectValues(Deno.consoleSize?.()) : undefined,
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
          (count === undefined || typeof count === "object" && count !== null)
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

  // If we're warming up, create a stdout/stderr stream that assumes a terminal (the most likely case).
  // If we're wrong at boot time, we'll recreate it.
  if (warmup || writer?.isTerminal()) {
    // These belong on tty.WriteStream(), but the TTY streams currently have
    // following problems:
    // 1. Using them here introduces a circular dependency.
    // 2. Creating a net.Socket() from a fd is not currently supported.
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
      stdin = new readStream(fd);
      break;
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
        readable: stdinType === "TTY" ? undefined : true,
        writable: stdinType === "TTY" ? undefined : false,
        readableHighWaterMark: stdinType === "TTY" ? 0 : undefined,
        allowHalfOpen: false,
        emitClose: false,
        autoDestroy: true,
        decodeStrings: false,
        read: _read,
      });

      if (stdinType !== "TTY") {
        // Make sure the stdin can't be `.end()`-ed
        stdin._writableState.ended = true;
      }

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

  // Allow users to overwrite isTTY for test isolation and terminal mocking.
  // This mirrors the stdout/stderr behavior added in #26130.
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
    io.stdin?.setRaw?.(enable);
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
