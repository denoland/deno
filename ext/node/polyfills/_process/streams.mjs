// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { Buffer } from "ext:deno_node/buffer.ts";
import {
  clearLine,
  clearScreenDown,
  cursorTo,
  moveCursor,
} from "ext:deno_node/internal/readline/callbacks.mjs";
import { Duplex, Readable, Writable } from "ext:deno_node/stream.ts";
import { isWindows } from "ext:deno_node/_util/os.ts";
import { fs as fsConstants } from "ext:deno_node/internal_binding/constants.ts";
import * as io from "ext:deno_io/12_io.js";

// https://github.com/nodejs/node/blob/00738314828074243c9a52a228ab4c68b04259ef/lib/internal/bootstrap/switches/is_main_thread.js#L41
export function createWritableStdioStream(writer, name) {
  const stream = new Writable({
    write(buf, enc, cb) {
      if (!writer) {
        this.destroy(
          new Error(`Deno.${name} is not available in this environment`),
        );
        return;
      }
      writer.writeSync(buf instanceof Uint8Array ? buf : Buffer.from(buf, enc));
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
  stream.fd = writer?.rid ?? -1;
  stream.destroySoon = stream.destroy;
  stream._isStdio = true;
  stream.once("close", () => writer?.close());
  Object.defineProperties(stream, {
    columns: {
      enumerable: true,
      configurable: true,
      get: () =>
        Deno.isatty?.(writer?.rid) ? Deno.consoleSize?.().columns : undefined,
    },
    rows: {
      enumerable: true,
      configurable: true,
      get: () =>
        Deno.isatty?.(writer?.rid) ? Deno.consoleSize?.().rows : undefined,
    },
    isTTY: {
      enumerable: true,
      configurable: true,
      get: () => Deno.isatty?.(writer?.rid),
    },
    getWindowSize: {
      enumerable: true,
      configurable: true,
      value: () =>
        Deno.isatty?.(writer?.rid)
          ? Object.values(Deno.consoleSize?.())
          : undefined,
    },
  });

  if (Deno.isatty?.(writer?.rid)) {
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

// TODO(PolarETech): This function should be replaced by
// `guessHandleType()` in "../internal_binding/util.ts".
// https://github.com/nodejs/node/blob/v18.12.1/src/node_util.cc#L257
function _guessStdinType(fd) {
  if (typeof fd !== "number" || fd < 0) return "UNKNOWN";
  if (Deno.isatty?.(fd)) return "TTY";

  try {
    const fileInfo = Deno.fstatSync?.(fd);

    // https://github.com/nodejs/node/blob/v18.12.1/deps/uv/src/unix/tty.c#L333
    if (!isWindows) {
      switch (fileInfo.mode & fsConstants.S_IFMT) {
        case fsConstants.S_IFREG:
        case fsConstants.S_IFCHR:
          return "FILE";
        case fsConstants.S_IFIFO:
          return "PIPE";
        case fsConstants.S_IFSOCK:
          // TODO(PolarETech): Need a better way to identify "TCP".
          // Currently, unable to exclude UDP.
          return "TCP";
        default:
          return "UNKNOWN";
      }
    }

    // https://github.com/nodejs/node/blob/v18.12.1/deps/uv/src/win/handle.c#L31
    if (fileInfo.isFile) {
      // TODO(PolarETech): Need a better way to identify a piped stdin on Windows.
      // On Windows, `Deno.fstatSync(rid).isFile` returns true even for a piped stdin.
      // Therefore, a piped stdin cannot be distinguished from a file by this property.
      // The mtime, atime, and birthtime of the file are "2339-01-01T00:00:00.000Z",
      // so use the property as a workaround.
      if (fileInfo.birthtime.valueOf() === 11644473600000) return "PIPE";
      return "FILE";
    }
  } catch (e) {
    // TODO(PolarETech): Need a better way to identify a character file on Windows.
    // "EISDIR" error occurs when stdin is "null" on Windows,
    // so use the error as a workaround.
    if (isWindows && e.code === "EISDIR") return "FILE";
  }

  return "UNKNOWN";
}

const _read = function (size) {
  const p = Buffer.alloc(size || 16 * 1024);
  io.stdin?.read(p).then((length) => {
    this.push(length === null ? null : p.slice(0, length));
  }, (error) => {
    this.destroy(error);
  });
};

/** https://nodejs.org/api/process.html#process_process_stdin */
// https://github.com/nodejs/node/blob/v18.12.1/lib/internal/bootstrap/switches/is_main_thread.js#L189
/** Create process.stdin */
export const initStdin = () => {
  const fd = io.stdin?.rid;
  let stdin;
  const stdinType = _guessStdinType(fd);

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
    case "TTY":
    case "PIPE":
    case "TCP": {
      // TODO(PolarETech):
      // For TTY, `new Duplex()` should be replaced `new tty.ReadStream()` if possible.
      // There are two problems that need to be resolved.
      // 1. Using them here introduces a circular dependency.
      // 2. Creating a tty.ReadStream() is not currently supported.
      // https://github.com/nodejs/node/blob/v18.12.1/lib/internal/bootstrap/switches/is_main_thread.js#L194
      // https://github.com/nodejs/node/blob/v18.12.1/lib/tty.js#L47

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
      break;
    }
    default: {
      // Provide a dummy contentless input for e.g. non-console
      // Windows applications.
      stdin = new Readable({ read() {} });
      stdin.push(null);
    }
  }

  stdin.on("close", () => io.stdin?.close());
  stdin.fd = io.stdin?.rid ?? -1;
  Object.defineProperty(stdin, "isTTY", {
    enumerable: true,
    configurable: true,
    get() {
      return Deno.isatty?.(Deno.stdin.rid);
    },
  });
  stdin._isRawMode = false;
  stdin.setRawMode = (enable) => {
    io.stdin?.setRaw?.(enable);
    stdin._isRawMode = enable;
    return stdin;
  };
  Object.defineProperty(stdin, "isRaw", {
    enumerable: true,
    configurable: true,
    get() {
      return stdin._isRawMode;
    },
  });

  return stdin;
};

