// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Ports lib/internal/cluster/utils.js.

// deno-lint-ignore-file no-explicit-any

import { primordials } from "ext:core/mod.js";

const { ReflectApply, SafeMap } = primordials;

const callbacks = new SafeMap();
let seq = 0;

export function sendHelper(
  proc: any,
  message: any,
  handle: any,
  cb?: (...args: any[]) => void,
): boolean {
  if (!proc.connected) {
    return false;
  }
  // Mark message as internal. Mirrors lib/internal/child_process.js prefix.
  message = { cmd: "NODE_CLUSTER", ...message, seq };
  if (typeof cb === "function") {
    callbacks.set(seq, cb);
  }
  seq += 1;
  return proc.send(message, handle);
}

// Returns an internalMessage listener that hands off normal messages to the
// callback but intercepts and redirects ACK messages.
export function internal(
  worker: any,
  cb: (this: any, message: any, handle?: any) => void,
) {
  return function onInternalMessage(this: any, message: any, _handle?: any) {
    if (message.cmd !== "NODE_CLUSTER") {
      return;
    }

    let fn: any = cb;

    if (message.ack !== undefined) {
      const callback = callbacks.get(message.ack);

      if (callback !== undefined) {
        fn = callback;
        callbacks.delete(message.ack);
      }
    }

    ReflectApply(fn, worker, arguments);
  };
}
