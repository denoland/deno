// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
import { op_tty_check_fd_permission, TTY } from "ext:core/ops";
import { Socket } from "node:net";
import { setReadStream } from "ext:deno_node/_process/streams.mjs";
import { WriteStream } from "ext:deno_node/internal/tty.js";

const {
  FunctionPrototypeCall,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
} = primordials;
const {
  ERR_INVALID_FD,
  ERR_TTY_INIT_FAILED,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

const { isatty } = core.loadExtScript("ext:deno_node/tty.js");

// ReadStream needs to be callable without `new` to match Node.js behavior.
// We use a wrapper function that delegates to the actual class.
// deno-lint-ignore no-explicit-any
function ReadStream(this: any, fd: number, options?: unknown) {
  if (!ObjectPrototypeIsPrototypeOf(ReadStream.prototype, this)) {
    // deno-lint-ignore no-explicit-any
    return new (ReadStream as any)(fd, options);
  }

  if (fd >> 0 !== fd || fd < 0) {
    throw new ERR_INVALID_FD(fd);
  }

  // Non-stdio fds require --allow-all
  op_tty_check_fd_permission(fd);

  // deno-lint-ignore no-explicit-any
  const ctx: any = {};
  const tty = new TTY(fd, ctx);
  if (ctx.code !== undefined) {
    throw new ERR_TTY_INIT_FAILED(ctx);
  }
  FunctionPrototypeCall(Socket, this, {
    readableHighWaterMark: 0,
    handle: tty,
    manualStart: true,
    // deno-lint-ignore no-explicit-any
    ...(options as any),
  });

  this.isRaw = false;
  this.isTTY = true;
}

ObjectSetPrototypeOf(ReadStream.prototype, Socket.prototype);
ObjectSetPrototypeOf(ReadStream, Socket);

// deno-lint-ignore no-explicit-any
(ReadStream as any).prototype.setRawMode = function setRawMode(
  this: { _handle: { setRawMode(flag: boolean): void }; isRaw: boolean },
  flag: boolean,
) {
  flag = !!flag;
  this._handle.setRawMode(flag);

  this.isRaw = flag;
  return this;
};

setReadStream(ReadStream);

export { isatty, ReadStream, WriteStream };
export default { isatty, WriteStream, ReadStream };
