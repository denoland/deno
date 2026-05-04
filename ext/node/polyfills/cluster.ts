// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
import { EventEmitter } from "node:events";
import { fork as childProcessFork } from "node:child_process";
import process from "node:process";
import { nextTick } from "ext:deno_node/_next_tick.ts";
const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");

const {
  Error,
  Number,
  NumberIsNaN,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectKeys,
  SafeArrayIterator,
  String,
} = primordials;

export const SCHED_NONE = 1;
export const SCHED_RR = 2;

/** A hash that stores the active worker objects, keyed by id field. Makes it
 * easy to loop through all the workers. It is only available in the primary
 * process. */
// deno-lint-ignore no-explicit-any
export const workers: Record<string, any> = {};

/** The settings object */
// deno-lint-ignore no-explicit-any
export const settings: Record<string, any> = {};

/** True if the process is a primary. This is determined by
 * the process.env.NODE_UNIQUE_ID. If process.env.NODE_UNIQUE_ID is undefined,
 * then isPrimary is true. */
export let isPrimary = true;
/** True if the process is not a primary (it is the negation of
 * cluster.isPrimary). */
export let isWorker = false;
/** Deprecated alias for cluster.isPrimary. details. */
export let isMaster = true;

/** A reference to the current worker object. Not available in the primary
 * process. */
export let worker: Worker | undefined = undefined;

/** The scheduling policy, either cluster.SCHED_RR for round-robin or
 * cluster.SCHED_NONE to leave it to the operating system. Matches Node:
 * SCHED_RR on POSIX, SCHED_NONE on Windows. */
export let schedulingPolicy: number | undefined = isWindows
  ? SCHED_NONE
  : SCHED_RR;

/** A Worker object contains all public information and method about a worker.
 * In the primary it can be obtained using cluster.workers. In a worker it can
 * be obtained using cluster.worker.
 */
export class Worker extends EventEmitter {
  id: number;
  // deno-lint-ignore no-explicit-any
  process: any;
  exitedAfterDisconnect = false;
  state = "none";

  // deno-lint-ignore no-explicit-any
  constructor(child: any, id: number) {
    super();
    this.id = id;
    this.process = child;

    if (child !== process) {
      child.on("error", (err: Error) => this.emit("error", err));
      child.on("exit", (code: number | null, signal: string | null) => {
        this.emit("exit", code, signal);
        delete workers[String(this.id)];
        cluster.emit("exit", this, code, signal);
      });
      child.on("disconnect", () => {
        this.emit("disconnect");
        cluster.emit("disconnect", this);
      });
      // deno-lint-ignore no-explicit-any
      child.on("message", (msg: any, handle: unknown) => {
        this.emit("message", msg, handle);
        cluster.emit("message", this, msg, handle);
      });
    }
  }

  // deno-lint-ignore no-explicit-any
  send(message: any, handle?: any, options?: any, callback?: any) {
    return this.process.send(message, handle, options, callback);
  }

  disconnect() {
    this.exitedAfterDisconnect = true;
    if (this.process.connected) {
      this.process.disconnect();
    }
    return this;
  }

  kill(signal: string = "SIGTERM") {
    this.exitedAfterDisconnect = true;
    if (this.process === process) {
      // In a worker, kill() means disconnect and exit. Wait for the IPC
      // disconnect to actually flush before exiting so any in-flight
      // messages aren't dropped (matches Node's lib/internal/cluster/child.js).
      // TODO(@divy-work): the worker-self path always exits cleanly with
      // exit(0) and ignores `signal`. Node delivers the signal via
      // `process.kill(process.pid, signal)` after disconnect, so the parent's
      // `'exit'` event sees the matching `signalCode`.
      if (!this.process.connected) {
        process.exit(0);
        return;
      }
      process.once("disconnect", () => process.exit(0));
      this.process.disconnect();
    } else {
      this.process.kill(signal);
    }
  }

  destroy(signal?: string) {
    this.kill(signal);
  }

  isConnected(): boolean {
    return this.process === process
      ? !!(process as { connected?: boolean }).connected
      : !!this.process.connected;
  }

  isDead(): boolean {
    if (this.process === process) return false;
    return this.process.exitCode !== null || this.process.signalCode !== null;
  }
}

let nextWorkerId = 0;

/** Calls .disconnect() on each worker in cluster.workers. */
export function disconnect(cb?: () => void) {
  const workerIds = ObjectKeys(workers);
  let remaining = workerIds.length;
  if (remaining === 0) {
    if (cb) nextTick(cb);
    return;
  }
  for (const id of new SafeArrayIterator(workerIds)) {
    const w = workers[id];
    w.once("disconnect", () => {
      remaining--;
      if (remaining === 0 && cb) cb();
    });
    w.disconnect();
  }
}

/** Spawn a new worker process. */
// deno-lint-ignore no-explicit-any
export function fork(env?: Record<string, any>): Worker {
  if (!isPrimary) {
    throw new Error("cluster.fork() can only be called from the primary");
  }
  const script = process.argv[1];
  if (!script) {
    throw new Error(
      "cluster.fork(): no script path available in process.argv",
    );
  }
  const id = ++nextWorkerId;
  // deno-lint-ignore no-explicit-any
  const childEnv: Record<string, any> = {
    ...process.env,
    ...(env || {}),
    NODE_UNIQUE_ID: String(id),
  };
  if (schedulingPolicy !== undefined) {
    childEnv.NODE_CLUSTER_SCHED_POLICY = String(schedulingPolicy);
  }

  const child = childProcessFork(script, [], {
    env: childEnv,
    silent: false,
  });
  const w = new Worker(child, id);
  w.state = "online";
  workers[String(id)] = w;
  cluster.emit("fork", w);
  // Emit "online" on next tick so handlers attached after fork() see it.
  nextTick(() => {
    w.emit("online");
    cluster.emit("online", w);
  });
  return w;
}

/** setupPrimary is used to change the default 'fork' behavior. Once called,
 * the settings will be present in cluster.settings.
 *
 * TODO(@divy-work): fork() does not yet honor the `exec`, `args`, `silent`,
 * `cwd`, or `serialization` settings populated here -- workers always run
 * `process.argv[1]` with the parent's stdio inherited.
 */
// deno-lint-ignore no-explicit-any
export function setupPrimary(options?: Record<string, any>) {
  if (options) {
    ObjectAssign(settings, options);
    if (typeof options.schedulingPolicy === "number") {
      schedulingPolicy = options.schedulingPolicy;
    }
  }
}
/** Deprecated alias for .setupPrimary(). */
export const setupMaster = setupPrimary;

const cluster = new EventEmitter() as EventEmitter & {
  isWorker: boolean;
  isMaster: boolean;
  isPrimary: boolean;
  Worker: typeof Worker;
  worker?: Worker;
  workers: Record<string, unknown>;
  settings: Record<string, unknown>;
  schedulingPolicy: number | undefined;
  setupPrimary(options?: Record<string, unknown>): void;
  setupMaster(options?: Record<string, unknown>): void;
  fork(env?: Record<string, unknown>): Worker;
  disconnect(cb?: () => void): void;
  SCHED_NONE: 1;
  SCHED_RR: 2;
};

cluster.Worker = Worker;
cluster.workers = workers;
cluster.settings = settings;
cluster.setupPrimary = setupPrimary;
cluster.setupMaster = setupMaster;
cluster.fork = fork;
cluster.disconnect = disconnect;
cluster.SCHED_NONE = SCHED_NONE;
cluster.SCHED_RR = SCHED_RR;

// `cluster.isPrimary`/`isWorker`/`isMaster` are read directly off the
// default-export object by user code. Use accessor properties so they reflect
// the runtime-detected values (set by `__initCluster`) rather than being
// frozen at module-load (snapshot) time.
ObjectDefineProperty(cluster, "isPrimary", {
  __proto__: null,
  get: () => isPrimary,
  enumerable: true,
  configurable: true,
});
ObjectDefineProperty(cluster, "isWorker", {
  __proto__: null,
  get: () => isWorker,
  enumerable: true,
  configurable: true,
});
ObjectDefineProperty(cluster, "isMaster", {
  __proto__: null,
  get: () => isMaster,
  enumerable: true,
  configurable: true,
});
ObjectDefineProperty(cluster, "worker", {
  __proto__: null,
  get: () => worker,
  enumerable: true,
  configurable: true,
});
ObjectDefineProperty(cluster, "schedulingPolicy", {
  __proto__: null,
  get: () => schedulingPolicy,
  set: (v: number | undefined) => {
    schedulingPolicy = v;
  },
  enumerable: true,
  configurable: true,
});

// Called from `02_init.js` only when NODE_UNIQUE_ID is present in the
// environment, so plain `deno run` never enters this path and never touches
// the env permission system. The caller passes both env values directly so
// this module never imports a permission-checked Deno API.
internals.__initCluster = (
  uniqueId: string,
  schedPolicyEnv: string | undefined,
) => {
  isPrimary = false;
  isWorker = true;
  isMaster = false;

  if (typeof schedPolicyEnv === "string" && schedPolicyEnv.length > 0) {
    const n = Number(schedPolicyEnv);
    if (!NumberIsNaN(n)) schedulingPolicy = n;
  }

  worker = new Worker(process, Number(uniqueId));
  worker.state = "online";
  workers[String(worker.id)] = worker;
};

export default cluster;
