// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Ports lib/internal/cluster/shared_handle.js.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file no-explicit-any prefer-primordials

import { core, primordials } from "ext:core/mod.js";
import * as net from "node:net";
const { codeMap } = core.loadExtScript(
  "ext:deno_node/internal_binding/uv.ts",
);

const { SafeMap } = primordials;

export function SharedHandle(
  this: any,
  key: string,
  address: string | null,
  { port, addressType, fd, flags }: any,
) {
  this.key = key;
  this.workers = new SafeMap();
  this.handle = null;
  this.errno = 0;

  let rval: any;
  if (addressType === "udp4" || addressType === "udp6") {
    // TODO(nathanwhit): wire dgram._createSocketHandle once dgram supports it.
    rval = codeMap.get("ENOTSUP")!;
  } else {
    rval = (net as any)._createServerHandle(
      address,
      port,
      addressType,
      fd,
      flags,
    );
  }

  if (typeof rval === "number") {
    this.errno = rval;
  } else {
    this.handle = rval;
    // Node's SharedHandle leaves the listen() to the worker -- libuv on
    // Node surfaces a deferred bind error from the worker's listen syscall
    // because the dup'd fd is the same kernel socket. In Deno, the worker
    // re-wraps the dup'd fd as a fresh tokio listener so the deferred error
    // and the LISTEN-state are not visible. Force-listen here so that the
    // primary's bind error is surfaced now, and the underlying fd is in
    // LISTEN state when the worker dups it.
    if (this.handle && typeof this.handle.listen === "function") {
      const backlog = port < 0 ? -1 : 511;
      const err = this.handle.listen(backlog);
      if (err) {
        this.errno = err;
        try {
          this.handle.close();
        } catch { /* ignore */ }
        this.handle = null;
      }
    }
  }
}

SharedHandle.prototype.add = function (
  this: any,
  worker: any,
  send: (errno: number, reply: null, handle: any) => void,
) {
  if (this.workers.has(worker.id)) {
    throw new Error("SharedHandle.add: worker already added");
  }
  this.workers.set(worker.id, worker);
  send(this.errno, null, this.handle);
};

SharedHandle.prototype.remove = function (this: any, worker: any) {
  if (!this.workers.has(worker.id)) {
    return false;
  }

  this.workers.delete(worker.id);

  if (this.workers.size !== 0) {
    return false;
  }

  this.handle.close();
  this.handle = null;
  return true;
};

SharedHandle.prototype.has = function (this: any, worker: any) {
  return this.workers.has(worker.id);
};

export default SharedHandle;
