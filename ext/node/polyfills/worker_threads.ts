// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { resolve, toFileUrl } from "ext:deno_node/path.ts";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { EventEmitter } from "ext:deno_node/events.ts";

const environmentData = new Map();
let threads = 0;

export interface WorkerOptions {
  // only for typings
  argv?: unknown[];
  env?: Record<string, unknown>;
  execArgv?: string[];
  stdin?: boolean;
  stdout?: boolean;
  stderr?: boolean;
  trackUnmanagedFds?: boolean;
  resourceLimits?: {
    maxYoungGenerationSizeMb?: number;
    maxOldGenerationSizeMb?: number;
    codeRangeSizeMb?: number;
    stackSizeMb?: number;
  };

  eval?: boolean;
  transferList?: Transferable[];
  workerData?: unknown;
}

const kHandle = Symbol("kHandle");
const PRIVATE_WORKER_THREAD_NAME = "$DENO_STD_NODE_WORKER_THREAD";
class _Worker extends EventEmitter {
  readonly threadId: number;
  readonly resourceLimits: Required<
    NonNullable<WorkerOptions["resourceLimits"]>
  > = {
    maxYoungGenerationSizeMb: -1,
    maxOldGenerationSizeMb: -1,
    codeRangeSizeMb: -1,
    stackSizeMb: 4,
  };
  private readonly [kHandle]: Worker;

  postMessage: Worker["postMessage"];

  constructor(specifier: URL | string, options?: WorkerOptions) {
    notImplemented("Worker");
    super();
    if (options?.eval === true) {
      specifier = `data:text/javascript,${specifier}`;
    } else if (typeof specifier === "string") {
      // @ts-ignore This API is temporarily disabled
      specifier = toFileUrl(resolve(specifier));
    }
    const handle = this[kHandle] = new Worker(
      specifier,
      {
        name: PRIVATE_WORKER_THREAD_NAME,
        type: "module",
      } as globalThis.WorkerOptions, // bypass unstable type error
    );
    handle.addEventListener(
      "error",
      (event) => this.emit("error", event.error || event.message),
    );
    handle.addEventListener(
      "messageerror",
      (event) => this.emit("messageerror", event.data),
    );
    handle.addEventListener(
      "message",
      (event) => this.emit("message", event.data),
    );
    handle.postMessage({
      environmentData,
      threadId: (this.threadId = ++threads),
      workerData: options?.workerData,
    }, options?.transferList || []);
    this.postMessage = handle.postMessage.bind(handle);
    this.emit("online");
  }

  terminate() {
    this[kHandle].terminate();
    this.emit("exit", 0);
  }

  readonly getHeapSnapshot = () =>
    notImplemented("Worker.prototype.getHeapSnapshot");
  // fake performance
  readonly performance = globalThis.performance;
}

export const isMainThread =
  // deno-lint-ignore no-explicit-any
  (globalThis as any).name !== PRIVATE_WORKER_THREAD_NAME;

// fake resourceLimits
export const resourceLimits = isMainThread ? {} : {
  maxYoungGenerationSizeMb: 48,
  maxOldGenerationSizeMb: 2048,
  codeRangeSizeMb: 0,
  stackSizeMb: 4,
};

const threadId = 0;
const workerData: unknown = null;

// Like https://github.com/nodejs/node/blob/48655e17e1d84ba5021d7a94b4b88823f7c9c6cf/lib/internal/event_target.js#L611
interface NodeEventTarget extends
  Pick<
    EventEmitter,
    "eventNames" | "listenerCount" | "emit" | "removeAllListeners"
  > {
  setMaxListeners(n: number): void;
  getMaxListeners(): number;
  // deno-lint-ignore no-explicit-any
  off(eventName: string, listener: (...args: any[]) => void): NodeEventTarget;
  // deno-lint-ignore no-explicit-any
  on(eventName: string, listener: (...args: any[]) => void): NodeEventTarget;
  // deno-lint-ignore no-explicit-any
  once(eventName: string, listener: (...args: any[]) => void): NodeEventTarget;
  addListener: NodeEventTarget["on"];
  removeListener: NodeEventTarget["off"];
}

type ParentPort = typeof self & NodeEventTarget;

// deno-lint-ignore no-explicit-any
const parentPort: ParentPort = null as any;

/*
if (!isMainThread) {
  // deno-lint-ignore no-explicit-any
  delete (globalThis as any).name;
  // deno-lint-ignore no-explicit-any
  const listeners = new WeakMap<(...args: any[]) => void, (ev: any) => any>();

  parentPort = self as ParentPort;
  parentPort.off = parentPort.removeListener = function (
    this: ParentPort,
    name,
    listener,
  ) {
    this.removeEventListener(name, listeners.get(listener)!);
    listeners.delete(listener);
    return this;
  };
  parentPort.on = parentPort.addListener = function (
    this: ParentPort,
    name,
    listener,
  ) {
    // deno-lint-ignore no-explicit-any
    const _listener = (ev: any) => listener(ev.data);
    listeners.set(listener, _listener);
    this.addEventListener(name, _listener);
    return this;
  };
  parentPort.once = function (this: ParentPort, name, listener) {
    // deno-lint-ignore no-explicit-any
    const _listener = (ev: any) => listener(ev.data);
    listeners.set(listener, _listener);
    this.addEventListener(name, _listener);
    return this;
  };

  // mocks
  parentPort.setMaxListeners = () => {};
  parentPort.getMaxListeners = () => Infinity;
  parentPort.eventNames = () => [""];
  parentPort.listenerCount = () => 0;

  parentPort.emit = () => notImplemented("parentPort.emit");
  parentPort.removeAllListeners = () =>
    notImplemented("parentPort.removeAllListeners");

  // Receive startup message
  [{ threadId, workerData, environmentData }] = await once(
    parentPort,
    "message",
  );

  // alias
  parentPort.addEventListener("offline", () => {
    parentPort.emit("close");
  });
}
*/

export function getEnvironmentData(key: unknown) {
  notImplemented("getEnvironmentData");
  return environmentData.get(key);
}

export function setEnvironmentData(key: unknown, value?: unknown) {
  notImplemented("setEnvironmentData");
  if (value === undefined) {
    environmentData.delete(key);
  } else {
    environmentData.set(key, value);
  }
}

// deno-lint-ignore no-explicit-any
const _MessagePort: typeof MessagePort = (globalThis as any).MessagePort;
const _MessageChannel: typeof MessageChannel =
  // deno-lint-ignore no-explicit-any
  (globalThis as any).MessageChannel;
export const BroadcastChannel = globalThis.BroadcastChannel;
export const SHARE_ENV = Symbol.for("nodejs.worker_threads.SHARE_ENV");
export function markAsUntransferable() {
  notImplemented("markAsUntransferable");
}
export function moveMessagePortToContext() {
  notImplemented("moveMessagePortToContext");
}
export function receiveMessageOnPort() {
  notImplemented("receiveMessageOnPort");
}
export {
  _MessageChannel as MessageChannel,
  _MessagePort as MessagePort,
  _Worker as Worker,
  parentPort,
  threadId,
  workerData,
};

export default {
  markAsUntransferable,
  moveMessagePortToContext,
  receiveMessageOnPort,
  MessagePort: _MessagePort,
  MessageChannel: _MessageChannel,
  BroadcastChannel,
  Worker: _Worker,
  getEnvironmentData,
  setEnvironmentData,
  SHARE_ENV,
  threadId,
  workerData,
  resourceLimits,
  parentPort,
  isMainThread,
};
