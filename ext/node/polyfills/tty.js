// Copyright 2018-2026 the Deno authors. MIT license.

import { op_node_is_tty } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";
const {
  FunctionPrototypeCall,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
} = primordials;

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

// ReadStream needs to be callable without `new` to match Node.js behavior.
function ReadStream(fd, options) {
  if (!ObjectPrototypeIsPrototypeOf(ReadStream.prototype, this)) {
    return new ReadStream(fd, options);
  }

  if (fd >> 0 !== fd || fd < 0) {
    throw new ERR_INVALID_FD(fd);
  }

  let handle;
  // For fd > 2 (PTY from NAPI modules like node-pty), create a TTYStream wrapper
  const isPty = fd > 2;
  if (isPty) {
    // Security: Only allow TTY file descriptors. This prevents access to
    // arbitrary fds (sockets, files, etc.) via tty.ReadStream/WriteStream.
    if (!op_node_is_tty(fd)) {
      throw new ERR_INVALID_FD(fd);
    }
    const rid = getRid(fd);
    const stream = new TTYStream(rid, fd);
    handle = new TTY(stream);
  } else {
    handle = new TTY(
      fd === 0 ? io.stdin : fd === 1 ? io.stdout : io.stderr,
    );
  }

  FunctionPrototypeCall(Socket, this, {
    readableHighWaterMark: 0,
    handle,
    manualStart: !isPty, // PTY streams should auto-start reading
    ...options,
  });

  this.isRaw = false;
  this.isTTY = true;
}

ObjectSetPrototypeOf(ReadStream.prototype, Socket.prototype);
ObjectSetPrototypeOf(ReadStream, Socket);

ReadStream.prototype.setRawMode = function setRawMode(flag) {
  flag = !!flag;
  this._handle.setRaw(flag);

  this.isRaw = flag;
  return this;
};

export { ReadStream };

setReadStream(ReadStream);

export { isatty, WriteStream };
export default { isatty, WriteStream, ReadStream };
