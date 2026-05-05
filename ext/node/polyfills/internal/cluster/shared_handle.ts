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
    // Match Node: leave listen() to the worker. We rely on `_createServerHandle`
    // (and below it `uv_tcp_bind`) reporting EADDRINUSE synchronously, so the
    // primary doesn't need to enter LISTEN state to discover bind errors.
    // Keeping the primary out of LISTEN avoids it sitting on the kernel
    // accept queue for the shared port (it never calls accept()) and avoids
    // surprising interactions with later bind() calls in the primary process
    // -- e.g. an ipv6Only listener on `::` blocking a sibling 0.0.0.0 bind.
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
