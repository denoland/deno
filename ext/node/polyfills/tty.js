// Copyright 2018-2026 the Deno authors. MIT license.

import {
  op_bootstrap_color_depth,
  op_node_is_tty,
  op_set_raw,
} from "ext:core/ops";
import { core, primordials } from "ext:core/mod.js";
const {
  Error,
} = primordials;
const { internalRidSymbol } = core;

import { ERR_INVALID_FD } from "ext:deno_node/internal/errors.ts";
import { TTY } from "ext:deno_node/internal_binding/tty_wrap.ts";
import { Socket } from "node:net";
import { setReadStream } from "ext:deno_node/_process/streams.mjs";
import * as io from "ext:deno_io/12_io.js";
import { getRid } from "ext:deno_node/internal/fs/fd_map.ts";

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

export class WriteStream extends Socket {
  constructor(fd) {
    if (fd >> 0 !== fd || fd < 0) {
      throw new ERR_INVALID_FD(fd);
    }

    let handle;
    if (fd > 2) {
      // For fd > 2 (PTY from NAPI modules), create a TTYStream wrapper
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
      manualStart: true,
    });

    try {
      const { columns, rows } = Deno.consoleSize();
      this.columns = columns;
      this.rows = rows;
    } catch {
      // consoleSize can fail if not a real TTY
      this.columns = 80;
      this.rows = 24;
    }
    this.isTTY = true;
  }

  /**
   * @param {number | Record<string, string>} [count]
   * @param {Record<string, string>} [env]
   * @returns {boolean}
   */
  hasColors(count, env) {
    if (
      env === undefined &&
      (count === undefined || typeof count === "object" && count !== null)
    ) {
      env = count;
      count = 16;
    }

    const depth = this.getColorDepth(env);
    return count <= 2 ** depth;
  }

  /**
   * @param {Record<string, string>} [env]
   * @returns {1 | 4 | 8 | 24}
   */
  getColorDepth(_env) {
    // TODO(@marvinhagemeister): Ignore env parameter.
    // Haven't seen it used anywhere, seems more done
    // to make testing easier in Node
    return op_bootstrap_color_depth();
  }
}

export { isatty };
export default { isatty, WriteStream, ReadStream };
