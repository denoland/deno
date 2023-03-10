// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { Socket } from "ext:deno_node/net.ts";

// Returns true when the given numeric fd is associated with a TTY and false otherwise.
function isatty(fd: number) {
  if (typeof fd !== "number") {
    return false;
  }
  try {
    return Deno.isatty(fd);
  } catch (_) {
    return false;
  }
}

// TODO(kt3k): Implement tty.ReadStream class
export class ReadStream extends Socket {
}
// TODO(kt3k): Implement tty.WriteStream class
export class WriteStream extends Socket {
}

export { isatty };
export default { isatty, WriteStream, ReadStream };
