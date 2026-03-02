// Copyright 2018-2026 the Deno authors. MIT license.

import { op_node_is_tty } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";
const { Error } = primordials;

import { ERR_INVALID_FD } from "ext:deno_node/internal/errors.ts";
import { TTY } from "ext:deno_node/internal_binding/tty_wrap.ts";
import { Socket } from "node:net";
import { setReadStream } from "ext:deno_node/_process/streams.mjs";
import * as io from "ext:deno_io/12_io.js";
import { WriteStream } from "ext:deno_node/internal/tty.js";

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

    // We only support `stdin`.
    if (fd != 0) throw new Error("Only fd 0 is supported.");

    const tty = new TTY(io.stdin);
    super({
      readableHighWaterMark: 0,
      handle: tty,
      manualStart: true,
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
