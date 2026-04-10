// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  FunctionPrototypeCall,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
} = primordials;

import {
  ERR_INVALID_FD,
  ERR_TTY_INIT_FAILED,
} from "ext:deno_node/internal/errors.ts";
import { op_tty_check_fd_permission, TTY } from "ext:core/ops";
import { Socket } from "node:net";
import {
  setReadStream,
  setWriteStream,
} from "ext:deno_node/_process/streams.mjs";
import { WriteStream } from "ext:deno_node/internal/tty.js";

// Returns true when the given numeric fd is associated with a TTY and false otherwise.
function isatty(fd) {
  if (typeof fd !== "number" || fd >> 0 !== fd || fd < 0) {
    return false;
  }
  return TTY.isTTY(fd);
}

// ReadStream needs to be callable without `new` to match Node.js behavior.
// We use a wrapper function that delegates to the actual class.
function ReadStream(fd, options) {
  if (!ObjectPrototypeIsPrototypeOf(ReadStream.prototype, this)) {
    return new ReadStream(fd, options);
  }

  if (fd >> 0 !== fd || fd < 0) {
    throw new ERR_INVALID_FD(fd);
  }

  // Non-stdio fds require --allow-all
  op_tty_check_fd_permission(fd);

  const ctx = {};
  const tty = new TTY(fd, ctx);
  if (ctx.code !== undefined) {
    throw new ERR_TTY_INIT_FAILED(ctx);
  }
  FunctionPrototypeCall(Socket, this, {
    readableHighWaterMark: 0,
    handle: tty,
    manualStart: true,
    ...options,
  });

  this.isRaw = false;
  this.isTTY = true;
}

ObjectSetPrototypeOf(ReadStream.prototype, Socket.prototype);
ObjectSetPrototypeOf(ReadStream, Socket);

ReadStream.prototype.setRawMode = function setRawMode(flag) {
  flag = !!flag;
  this._handle.setRawMode(flag);

  this.isRaw = flag;
  return this;
};

export { ReadStream };

setReadStream(ReadStream);
setWriteStream(WriteStream);

export { isatty, WriteStream };
export default { isatty, WriteStream, ReadStream };
