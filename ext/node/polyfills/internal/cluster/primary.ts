// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Ports lib/internal/cluster/primary.js. The primary-side state is wired
// onto the cluster EventEmitter passed in by `cluster.ts`. Only one of
// `primary.init` or `child.init` runs in a given process; the choice is
// driven by NODE_UNIQUE_ID through `cluster.ts`'s dispatch.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file no-explicit-any prefer-primordials

import { core, primordials } from "ext:core/mod.js";
import { EventEmitter } from "node:events";
import { fork as childProcessFork } from "node:child_process";
import * as path from "node:path";
import process from "node:process";
const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");

import { Worker } from "ext:deno_node/internal/cluster/worker.ts";
import { internal, sendHelper } from "ext:deno_node/internal/cluster/utils.ts";
import { RoundRobinHandle } from "ext:deno_node/internal/cluster/round_robin_handle.ts";
import { SharedHandle } from "ext:deno_node/internal/cluster/shared_handle.ts";

const {
  ArrayPrototypeSlice,
  ObjectKeys,
  ObjectValues,
  SafeMap,
} = primordials;

const SCHED_NONE = 1;
const SCHED_RR = 2;

let initialized = false;

// Initialize primary-side state and methods on the shared cluster object.
// Mirrors lib/internal/cluster/primary.js's top-level setup.
export function init(cluster: any) {
  if (initialized) return;
  initialized = true;

  const intercom = new EventEmitter();
  const handles = new SafeMap();

  cluster.isWorker = false;
  cluster.isMaster = true;
  cluster.isPrimary = true;
  cluster.Worker = Worker;
  cluster.workers = {};
  cluster.settings = {};
  cluster.SCHED_NONE = SCHED_NONE;
  cluster.SCHED_RR = SCHED_RR;

  let ids = 0;
  let setupCalled = false;

  // Read the policy from env, mirroring lib/internal/cluster/primary.js.
  let schedulingPolicy: number;
  const env = (process as any).env.NODE_CLUSTER_SCHED_POLICY;
  if (env === "rr") {
    schedulingPolicy = SCHED_RR;
  } else if (env === "none") {
    schedulingPolicy = SCHED_NONE;
  } else if (isWindows) {
    schedulingPolicy = SCHED_NONE;
  } else {
    schedulingPolicy = SCHED_RR;
  }
  cluster.schedulingPolicy = schedulingPolicy;

  cluster.setupPrimary = function (options?: any) {
    const settings = {
      args: ArrayPrototypeSlice(process.argv, 2),
      exec: process.argv[1],
      execArgv: process.execArgv,
      silent: false,
      ...cluster.settings,
      ...options,
    };

    cluster.settings = settings;

    if (setupCalled === true) {
      return process.nextTick(setupSettingsNT, settings);
    }

    setupCalled = true;
    schedulingPolicy = cluster.schedulingPolicy;
    if (schedulingPolicy !== SCHED_NONE && schedulingPolicy !== SCHED_RR) {
      throw new Error(`Bad cluster.schedulingPolicy: ${schedulingPolicy}`);
    }

    process.nextTick(setupSettingsNT, settings);
  };
  cluster.setupMaster = cluster.setupPrimary;

  function setupSettingsNT(settings: any) {
    cluster.emit("setup", settings);
  }

  function createWorkerProcess(id: number, env: any) {
    const workerEnv: any = {
      ...(process as any).env,
      ...env,
      NODE_UNIQUE_ID: `${id}`,
    };
    if (schedulingPolicy === SCHED_RR) {
      workerEnv.NODE_CLUSTER_SCHED_POLICY = "rr";
    } else if (schedulingPolicy === SCHED_NONE) {
      workerEnv.NODE_CLUSTER_SCHED_POLICY = "none";
    }

    const execArgv = [...(cluster.settings.execArgv || [])];

    return childProcessFork(cluster.settings.exec, cluster.settings.args, {
      cwd: cluster.settings.cwd,
      env: workerEnv,
      serialization: cluster.settings.serialization,
      silent: cluster.settings.silent,
      windowsHide: cluster.settings.windowsHide,
      execArgv,
      stdio: cluster.settings.stdio,
      gid: cluster.settings.gid,
      uid: cluster.settings.uid,
    });
  }

  function removeWorker(worker: any) {
    if (!worker) return;
    delete cluster.workers[worker.id];

    if (ObjectKeys(cluster.workers).length === 0) {
      intercom.emit("disconnect");
    }
  }

  function removeHandlesForWorker(worker: any) {
    if (!worker) return;

    for (const [key, handle] of handles) {
      if (handle.remove(worker)) {
        handles.delete(key);
      }
    }
  }

  cluster.fork = function (env?: any) {
    cluster.setupPrimary();
    const id = ++ids;
    const workerProcess = createWorkerProcess(id, env);
    const worker = new (Worker as any)({
      id,
      process: workerProcess,
    });

    worker.on("message", function (this: any, message: any, handle: any) {
      cluster.emit("message", this, message, handle);
    });

    worker.process.once("exit", (exitCode: any, signalCode: any) => {
      if (!worker.isConnected()) {
        removeHandlesForWorker(worker);
        removeWorker(worker);
      }

      worker.exitedAfterDisconnect = !!worker.exitedAfterDisconnect;
      worker.state = "dead";
      worker.emit("exit", exitCode, signalCode);
      cluster.emit("exit", worker, exitCode, signalCode);
    });

    worker.process.once("disconnect", () => {
      removeHandlesForWorker(worker);

      if (worker.isDead()) {
        removeWorker(worker);
      }

      worker.exitedAfterDisconnect = !!worker.exitedAfterDisconnect;
      worker.state = "disconnected";
      worker.emit("disconnect");
      cluster.emit("disconnect", worker);
    });

    worker.process.on("internalMessage", internal(worker, onmessage));
    process.nextTick(emitForkNT, worker);
    cluster.workers[worker.id] = worker;
    return worker;
  };

  function emitForkNT(worker: any) {
    cluster.emit("fork", worker);
  }

  cluster.disconnect = function (cb?: () => void) {
    const workers = ObjectValues(cluster.workers);

    if (workers.length === 0) {
      process.nextTick(() => intercom.emit("disconnect"));
    } else {
      for (const worker of workers) {
        if ((worker as any).isConnected()) {
          (worker as any).disconnect();
        }
      }
    }

    if (typeof cb === "function") {
      intercom.once("disconnect", cb);
    }
  };

  const methodMessageMapping: Record<
    string,
    (worker: any, message: any) => void
  > = {
    close,
    exitedAfterDisconnect,
    listening,
    online,
    queryServer,
  };

  function onmessage(this: any, message: any, _handle?: any) {
    const fn = methodMessageMapping[message.act];

    if (typeof fn === "function") {
      fn(this, message);
    }
  }

  function online(worker: any) {
    worker.state = "online";
    worker.emit("online");
    cluster.emit("online", worker);
  }

  function exitedAfterDisconnect(worker: any, message: any) {
    worker.exitedAfterDisconnect = true;
    send(worker, { ack: message.seq });
  }

  function queryServer(worker: any, message: any) {
    if (worker.exitedAfterDisconnect) {
      return;
    }

    const key = `${message.address}:${message.port}:${message.addressType}:` +
      `${message.fd}` + (message.port === 0 ? `:${message.index}` : "");
    const cachedHandle = handles.get(key);
    let handle: any;
    if (cachedHandle && !cachedHandle.has(worker)) {
      handle = cachedHandle;
    }

    if (handle === undefined) {
      let address = message.address;

      if (
        message.port < 0 && typeof address === "string" && !isWindows
      ) {
        address = path.relative(process.cwd(), address);
        if (message.address.length < address.length) {
          address = message.address;
        }
      }

      if (
        schedulingPolicy !== SCHED_RR ||
        message.addressType === "udp4" ||
        message.addressType === "udp6"
      ) {
        handle = new (SharedHandle as any)(key, address, message);
      } else {
        handle = new (RoundRobinHandle as any)(key, address, message);
      }

      if (!cachedHandle) {
        handles.set(key, handle);
      }
    }

    handle.data ||= message.data;

    handle.add(worker, (errno: any, reply: any, serverHandle: any) => {
      if (!errno) {
        handles.set(key, handle);
      }
      const cur = handles.get(key);
      const data = cur ? cur.data : undefined;
      if (!cachedHandle && errno) {
        handles.delete(key);
      }

      send(worker, {
        errno,
        key,
        ack: message.seq,
        data,
        ...reply,
      }, serverHandle);
    });
  }

  function listening(worker: any, message: any) {
    const info = {
      addressType: message.addressType,
      address: message.address,
      port: message.port,
      fd: message.fd,
    };

    worker.state = "listening";
    worker.emit("listening", info);
    cluster.emit("listening", worker, info);
  }

  function close(worker: any, message: any) {
    const key = message.key;
    const handle = handles.get(key);

    if (handle && handle.remove(worker)) {
      handles.delete(key);
    }
  }

  function send(worker: any, message: any, handle?: any, cb?: any) {
    return sendHelper(worker.process, message, handle, cb);
  }

  // Extend generic Worker with primary-specific methods.
  (Worker as any).prototype.disconnect = function (this: any) {
    this.exitedAfterDisconnect = true;
    send(this, { act: "disconnect" });
    removeHandlesForWorker(this);
    removeWorker(this);
    return this;
  };

  (Worker as any).prototype.destroy = function (this: any, signo?: string) {
    const signal = signo || "SIGTERM";
    this.process.kill(signal);
  };
}
