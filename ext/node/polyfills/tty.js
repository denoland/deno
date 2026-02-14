// Copyright 2018-2026 the Deno authors. MIT license.

import { op_node_is_tty, op_set_raw } from "ext:core/ops";
import { core } from "ext:core/mod.js";

import { ERR_INVALID_FD } from "ext:deno_node/internal/errors.ts";
import { TTY } from "ext:deno_node/internal_binding/tty_wrap.ts";
import { Socket } from "node:net";
import { setReadStream } from "ext:deno_node/_process/streams.mjs";
import * as io from "ext:deno_io/12_io.js";
import { WriteStream } from "ext:deno_node/internal/tty.js";
import { getRid } from "ext:deno_node/internal/fs/fd_map.ts";

const { internalRidSymbol } = core;

// Helper class to wrap a resource ID as a stream-like object.
// Used for PTY file descriptors (fd > 2) that come from NAPI modules like node-pty.
// Similar to Stdin/Stdout/Stderr classes in io module.
class TTYStream {
  #rid;
  #ref = true;
  #opPromise;

  constructor(rid) {
    this.#rid = rid;
  }

  get [internalRidSymbol]() {
    return this.#rid;
  }

  get rid() {
    return this.#rid;
  }

  async read(p) {
    if (p.length === 0) return 0;
    this.#opPromise = core.read(this.#rid, p);
    if (!this.#ref) {
      core.unrefOpPromise(this.#opPromise);
    }
    const nread = await this.#opPromise;
    return nread === 0 ? null : nread;
  }

  readSync(p) {
    if (p.length === 0) return 0;
    const nread = core.readSync(this.#rid, p);
    return nread === 0 ? null : nread;
  }

  write(p) {
    return core.write(this.#rid, p);
  }

  writeSync(p) {
    return core.writeSync(this.#rid, p);
  }

  close() {
    core.tryClose(this.#rid);
  }

  setRaw(mode, options = { __proto__: null }) {
    const cbreak = !!(options.cbreak ?? false);
    op_set_raw(this.#rid, mode, cbreak);
  }

  isTerminal() {
    return core.isTerminal(this.#rid);
  }

  [io.REF]() {
    this.#ref = true;
    if (this.#opPromise) {
      core.refOpPromise(this.#opPromise);
    }
  }

  [io.UNREF]() {
    this.#ref = false;
    if (this.#opPromise) {
      core.unrefOpPromise(this.#opPromise);
    }
  }
}

// Returns true when the given numeric fd is associated with a TTY and false otherwise.
function isatty(fd) {
  if (typeof fd !== "number" || fd >> 0 !== fd || fd < 0) {
    return false;
  }
  return op_node_is_tty(fd);
}

export class ReadStream extends Socket {
  constructor(fd, options) {
    if (fd >> 0 !== fd || fd < 0) {
      throw new ERR_INVALID_FD(fd);
    }

    let handle;
    // For fd > 2 (PTY from NAPI modules like node-pty), create a TTYStream wrapper
    const isPty = fd > 2;
    if (isPty) {
      // Security: Only allow TTY file descriptors. This prevents access to
      // arbitrary fds (sockets, files, etc.) via tty.ReadStream/WriteStream.
      // PTY devices from node-pty are real TTYs so isatty() returns true.
      if (!op_node_is_tty(fd)) {
        throw new ERR_INVALID_FD(fd);
      }
      // Get the rid from the fd map (will dup and create resource if needed)
      const rid = getRid(fd);
      const stream = new TTYStream(rid);
      handle = new TTY(stream);
    } else {
      // For stdin/stdout/stderr, use the built-in handles
      handle = new TTY(
        fd === 0 ? io.stdin : fd === 1 ? io.stdout : io.stderr,
      );
    }
    super({
      readableHighWaterMark: 0,
      handle,
      manualStart: !isPty, // PTY streams should auto-start reading
      ...options,
    });

    this.isRaw = false;
    this.isTTY = true;
  }

  setRawMode(flag) {
    flag = !!flag;
    this._handle.setRaw(flag);

    this.isRaw = flag;
    return this;
  }
}

setReadStream(ReadStream);

export { isatty, WriteStream };
export default { isatty, WriteStream, ReadStream };
