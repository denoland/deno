// Copyright 2018-2026 the Deno authors. MIT license.

import { op_node_is_tty } from "ext:core/ops";

import { ERR_INVALID_FD } from "ext:deno_node/internal/errors.ts";
import { TTY } from "ext:deno_node/internal_binding/tty_wrap.ts";
import { Socket } from "node:net";
import { setReadStream } from "ext:deno_node/_process/streams.mjs";
import * as io from "ext:deno_io/12_io.js";
import { TTYStream, WriteStream } from "ext:deno_node/internal/tty.js";
import { getRid } from "ext:deno_node/internal/fs/fd_map.ts";

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
