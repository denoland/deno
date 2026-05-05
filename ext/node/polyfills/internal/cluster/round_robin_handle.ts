// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Ports lib/internal/cluster/round_robin_handle.js.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file no-explicit-any prefer-primordials

import { primordials } from "ext:core/mod.js";
import * as net from "node:net";
import { sendHelper } from "ext:deno_node/internal/cluster/utils.ts";
import {
  append,
  init,
  isEmpty,
  peek,
  remove,
} from "ext:deno_node/internal/cluster/linkedlist.ts";
import {
  constants as TCPConstants,
  setupListenWrap as setupTCPListenWrap,
  TCP,
} from "ext:deno_node/internal_binding/tcp_wrap.ts";
import {
  Pipe,
  setupListenWrap as setupPipeListenWrap,
} from "ext:deno_node/internal_binding/pipe_wrap.ts";

const { ArrayIsArray, Boolean, SafeMap } = primordials;

export function RoundRobinHandle(
  this: any,
  key: string,
  address: string | null,
  { port, fd, flags, backlog, readableAll, writableAll }: any,
) {
  this.key = key;
  this.all = new SafeMap();
  this.free = new SafeMap();
  this.handles = init({ __proto__: null });
  this.handle = null;
  this.server = (net as any).createServer(() => {
    throw new Error("RoundRobinHandle should never accept connections");
  });

  if (fd >= 0) {
    this.server.listen({ fd, backlog });
  } else if (port >= 0) {
    this.server.listen({
      port,
      host: address,
      // Currently, net module only supports `ipv6Only` option in `flags`.
      ipv6Only: Boolean(flags & TCPConstants.UV_TCP_IPV6ONLY),
      backlog,
    });
  } else {
    this.server.listen({
      path: address,
      backlog,
      readableAll,
      writableAll,
    }); // UNIX socket path.
  }
  this.server.once("listening", () => {
    this.handle = this.server._handle;
    // Replace the listen-wrap that net.Server set up: it captures the
    // onconnection at wrap time, so just assigning a new onconnection
    // (as Node does) would skip our accept-wrap step in Deno's TCP/Pipe
    // bindings. Re-run setupListenWrap with the new user callback.
    this.handle.onconnection = (err: number, handle: any) =>
      this.distribute(err, handle);
    if (this.handle instanceof TCP) {
      setupTCPListenWrap(this.handle);
    } else if (this.handle instanceof Pipe) {
      setupPipeListenWrap(this.handle);
    }
    this.server._handle = null;
    this.server = null;
  });
}

RoundRobinHandle.prototype.add = function (
  this: any,
  worker: any,
  send: (errno: number | null, reply: any, handle: any) => void,
) {
  if (this.all.has(worker.id)) {
    throw new Error("RoundRobinHandle.add: worker already added");
  }
  this.all.set(worker.id, worker);

  const done = () => {
    if (this.handle.getsockname) {
      const out: any = {};
      this.handle.getsockname(out);
      send(null, { sockname: out }, null);
    } else {
      send(null, null, null); // UNIX socket.
    }

    this.handoff(worker); // In case there are connections pending.
  };

  if (this.server === null) {
    return done();
  }

  // Still busy binding.
  this.server.once("listening", done);
  this.server.once("error", (err: any) => {
    send(err.errno, null, null);
  });
};

RoundRobinHandle.prototype.remove = function (this: any, worker: any) {
  const existed = this.all.delete(worker.id);

  if (!existed) {
    return false;
  }

  this.free.delete(worker.id);

  if (this.all.size !== 0) {
    return false;
  }

  while (!isEmpty(this.handles)) {
    const handle = peek(this.handles);
    handle.close();
    remove(handle);
  }

  this.handle.close();
  this.handle = null;
  return true;
};

RoundRobinHandle.prototype.distribute = function (
  this: any,
  err: number,
  handle: any,
) {
  // If `accept` fails just skip it (handle is undefined).
  if (err) {
    return;
  }
  append(this.handles, handle);
  // Destructures the first `[key, value]` entry from the SafeMap iterator;
  // `workerEntry` is `undefined` if `this.free` is empty. The `ArrayIsArray`
  // guard (inherited from Node's port) is just a non-empty check -- map
  // entries are always 2-tuples.
  const [workerEntry] = this.free;

  if (ArrayIsArray(workerEntry)) {
    const { 0: workerId, 1: worker } = workerEntry;
    this.free.delete(workerId);
    this.handoff(worker);
  }
};

RoundRobinHandle.prototype.handoff = function (this: any, worker: any) {
  if (!this.all.has(worker.id)) {
    return; // Worker is closing (or has closed) the server.
  }

  const handle = peek(this.handles);

  if (handle === null) {
    this.free.set(worker.id, worker); // Add to ready queue again.
    return;
  }

  remove(handle);

  const message = { act: "newconn", key: this.key };

  sendHelper(worker.process, message, handle, (reply: any) => {
    if (reply.accepted) {
      handle.close();
    } else {
      this.distribute(0, handle); // Worker is shutting down. Send to another.
    }

    this.handoff(worker);
  });
};

RoundRobinHandle.prototype.has = function (this: any, worker: any) {
  return this.all.has(worker.id);
};

export default RoundRobinHandle;
