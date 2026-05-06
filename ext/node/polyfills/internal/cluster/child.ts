// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Ports lib/internal/cluster/child.js. The child-side state is wired onto
// the cluster EventEmitter passed in by `cluster.ts`. Only one of
// `primary.init` or `child.init` runs in a given process; the choice is
// driven by NODE_UNIQUE_ID through `cluster.ts`'s dispatch.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file no-explicit-any prefer-primordials

import { core, primordials } from "ext:core/mod.js";
import * as path from "node:path";
import process from "node:process";
const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");
import { Worker } from "ext:deno_node/internal/cluster/worker.ts";
import { internal, sendHelper } from "ext:deno_node/internal/cluster/utils.ts";
const { ownerSymbol } = core.loadExtScript(
  "ext:deno_node/internal/async_hooks.ts",
);

const {
  ArrayPrototypeJoin,
  FunctionPrototype,
  FunctionPrototypeCall,
  ObjectAssign,
  ReflectApply,
  SafeMap,
  SafeSet,
} = primordials;

let initialized = false;

export function init(cluster: any) {
  if (initialized) return;
  initialized = true;

  const handles = new SafeMap();
  const indexes = new SafeMap();
  const noop = FunctionPrototype;

  cluster.isWorker = true;
  cluster.isMaster = false;
  cluster.isPrimary = false;
  cluster.worker = null;
  cluster.Worker = Worker;
  // Drop primary-only methods so user code that branches on their
  // presence (e.g. `if (cluster.fork) ...`) sees the worker shape.
  cluster.fork = undefined;
  cluster.setupPrimary = undefined;
  cluster.setupMaster = undefined;
  cluster.workers = undefined;
  cluster.settings = undefined;

  cluster._setupWorker = function () {
    const worker = new (Worker as any)({
      id: +(process as any).env.NODE_UNIQUE_ID | 0,
      process,
      state: "online",
    });

    cluster.worker = worker;

    // Match Node: remove NODE_UNIQUE_ID after the worker is set up so it
    // doesn't accidentally get inherited by child processes the worker spawns.
    try {
      delete (process as any).env.NODE_UNIQUE_ID;
    } catch { /* ignore */ }

    process.once("disconnect", () => {
      worker.emit("disconnect");

      if (!worker.exitedAfterDisconnect) {
        process.exit(0);
      }
    });

    process.on("internalMessage", internal(worker, onmessage));
    send({ act: "online" });

    function onmessage(this: any, message: any, handle: any) {
      if (message.act === "newconn") {
        onconnection(message, handle);
      } else if (message.act === "disconnect") {
        FunctionPrototypeCall(_disconnect, worker, true);
      }
    }
  };

  cluster._getServer = function (obj: any, options: any, cb: any) {
    let address = options.address;

    if (
      options.port < 0 && typeof address === "string" && !isWindows
    ) {
      address = path.resolve(address);
    }

    const indexesKey = ArrayPrototypeJoin(
      [address, options.port, options.addressType, options.fd],
      ":",
    );

    let indexSet = indexes.get(indexesKey);

    if (indexSet === undefined) {
      indexSet = { nextIndex: 0, set: new SafeSet() };
      indexes.set(indexesKey, indexSet);
    }
    const index = indexSet.nextIndex++;
    indexSet.set.add(index);

    const message: any = {
      act: "queryServer",
      index,
      data: null,
      ...options,
    };

    message.address = address;

    if (obj._getServerData) {
      message.data = obj._getServerData();
    }

    send(message, (reply: any, handle: any) => {
      if (typeof obj._setServerData === "function") {
        obj._setServerData(reply.data);
      }

      if (handle) {
        shared(reply, { handle, indexesKey, index }, cb);
      } else {
        rr(reply, { indexesKey, index }, cb);
      }
    });

    obj.once("listening", () => {
      if (!indexes.has(indexesKey)) {
        return;
      }
      cluster.worker.state = "listening";
      const addr = obj.address();
      message.act = "listening";
      message.port = (addr?.port) || options.port;
      send(message);
    });
  };

  function removeIndexesKey(indexesKey: any, index: any) {
    const indexSet = indexes.get(indexesKey);
    if (!indexSet) {
      return;
    }

    indexSet.set.delete(index);
    if (indexSet.set.size === 0) {
      indexes.delete(indexesKey);
    }
  }

  function shared(
    message: any,
    { handle, indexesKey, index }: any,
    cb: any,
  ) {
    const key = message.key;
    const close = handle.close;

    handle.close = function (this: any) {
      send({ act: "close", key });
      handles.delete(key);
      removeIndexesKey(indexesKey, index);
      return ReflectApply(close, handle, arguments);
    };
    handles.set(key, handle);
    cb(message.errno, handle);
  }

  function rr(
    message: any,
    { indexesKey, index }: any,
    cb: any,
  ) {
    if (message.errno) {
      return cb(message.errno, null);
    }

    let key = message.key;
    let fakeHandle: any = null;

    function ref() {
      if (!fakeHandle) {
        fakeHandle = setInterval(noop as any, 2 ** 31 - 1);
      }
    }

    function unref() {
      if (fakeHandle) {
        clearInterval(fakeHandle);
        fakeHandle = null;
      }
    }

    function listen(_backlog: number) {
      return 0;
    }

    function close() {
      if (key === undefined) {
        return;
      }
      unref();
      send({ act: "close", key });
      handles.delete(key);
      removeIndexesKey(indexesKey, index);
      key = undefined;
    }

    function getsockname(out: any) {
      if (key) {
        ObjectAssign(out, message.sockname);
      }
      return 0;
    }

    const handle: any = { close, listen, ref, unref };
    handle.ref();
    if (message.sockname) {
      handle.getsockname = getsockname;
    }

    handles.set(key, handle);
    cb(0, handle);
  }

  function onconnection(message: any, handle: any) {
    const key = message.key;
    const server = handles.get(key);
    let accepted = server !== undefined;

    // Match Node: when the worker's net.Server has hit its maxConnections,
    // refuse the handoff so the primary can route the connection to a
    // different worker. dropMaxConnection bypasses this -- the worker accepts
    // the handle and then drops it.
    if (accepted && server[ownerSymbol]) {
      const self = server[ownerSymbol];
      if (
        self.maxConnections != null &&
        self._connections >= self.maxConnections &&
        !self.dropMaxConnection
      ) {
        accepted = false;
      }
    }

    send({ ack: message.seq, accepted });

    if (accepted) {
      server.onconnection(0, handle);
    } else {
      handle.close();
    }
  }

  function send(message: any, cb?: any) {
    return sendHelper(process, message, null, cb);
  }

  function _disconnect(this: any, primaryInitiated?: boolean) {
    this.exitedAfterDisconnect = true;
    let waitingCount = 1;

    function checkWaitingCount() {
      waitingCount--;

      if (waitingCount === 0) {
        if (primaryInitiated) {
          (process as any).disconnect();
        } else {
          send(
            { act: "exitedAfterDisconnect" },
            () => (process as any).disconnect(),
          );
        }
      }
    }

    for (const handle of handles.values()) {
      waitingCount++;
      // Match Node: prefer closing through the owning net.Server (which
      // ends connections gracefully) over the bare cluster fake handle's
      // synchronous close. The owner takes a callback, the fake handle
      // currently doesn't, so without this the worker would never call
      // checkWaitingCount and never disconnect.
      if (handle[ownerSymbol]) {
        handle[ownerSymbol].close(checkWaitingCount);
      } else {
        handle.close(checkWaitingCount);
      }
    }

    handles.clear();
    checkWaitingCount();
  }

  // Extend generic Worker with worker-specific methods.
  (Worker as any).prototype.disconnect = function (this: any) {
    if (this.state !== "disconnecting" && this.state !== "destroying") {
      this.state = "disconnecting";
      FunctionPrototypeCall(_disconnect, this);
    }

    return this;
  };

  (Worker as any).prototype.destroy = function (this: any) {
    if (this.state === "destroying") {
      return;
    }

    this.exitedAfterDisconnect = true;
    if (!this.isConnected()) {
      process.exit(0);
    } else {
      this.state = "destroying";
      send(
        { act: "exitedAfterDisconnect" },
        () => (process as any).disconnect(),
      );
      process.once("disconnect", () => process.exit(0));
    }
  };

  // disconnect-with-callback variant exposed at the cluster level.
  cluster.disconnect = function (cb?: () => void) {
    if (typeof cb === "function") {
      process.once("disconnect", cb);
    }
    if (cluster.worker) {
      cluster.worker.disconnect();
    }
  };
}
