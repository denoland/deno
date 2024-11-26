// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
import {
  op_create_worker,
  op_host_post_message,
  op_host_recv_ctrl,
  op_host_recv_message,
  op_host_terminate_worker,
  op_message_port_recv_message_sync,
  op_worker_threads_filename,
} from "ext:core/ops";
import {
  deserializeJsMessageData,
  MessageChannel,
  MessagePort,
  MessagePortIdSymbol,
  MessagePortPrototype,
  MessagePortReceiveMessageOnPortSymbol,
  nodeWorkerThreadCloseCb,
  refMessagePort,
  serializeJsMessageData,
  unrefPollForMessages,
} from "ext:deno_web/13_message_port.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { EventEmitter } from "node:events";
import { BroadcastChannel } from "ext:deno_broadcast_channel/01_broadcast_channel.js";
import process from "node:process";

const { JSONParse, JSONStringify, ObjectPrototypeIsPrototypeOf } = primordials;
const {
  Error,
  ObjectHasOwn,
  PromiseResolve,
  SafeSet,
  Symbol,
  SymbolFor,
  SymbolIterator,
  StringPrototypeTrim,
  SafeWeakMap,
  SafeMap,
  TypeError,
} = primordials;

const debugWorkerThreads = false;
function debugWT(...args) {
  if (debugWorkerThreads) {
    // deno-lint-ignore prefer-primordials no-console
    console.log(...args);
  }
}

interface WorkerOnlineMsg {
  type: "WORKER_ONLINE";
}

function isWorkerOnlineMsg(data: unknown): data is WorkerOnlineMsg {
  return typeof data === "object" && data !== null &&
    ObjectHasOwn(data, "type") &&
    (data as { "type": unknown })["type"] === "WORKER_ONLINE";
}

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
  // deno-lint-ignore prefer-primordials
  eval?: boolean;
  transferList?: Transferable[];
  workerData?: unknown;
  name?: string;
}

const privateWorkerRef = Symbol("privateWorkerRef");
class NodeWorker extends EventEmitter {
  #id = 0;
  #name = "";
  #refCount = 1;
  #messagePromise = undefined;
  #controlPromise = undefined;
  #workerOnline = false;
  // "RUNNING" | "CLOSED" | "TERMINATED"
  // "TERMINATED" means that any controls or messages received will be
  // discarded. "CLOSED" means that we have received a control
  // indicating that the worker is no longer running, but there might
  // still be messages left to receive.
  #status = "RUNNING";

  // https://nodejs.org/api/worker_threads.html#workerthreadid
  threadId = this.#id;
  // https://nodejs.org/api/worker_threads.html#workerresourcelimits
  resourceLimits: Required<
    NonNullable<WorkerOptions["resourceLimits"]>
  > = {
    maxYoungGenerationSizeMb: -1,
    maxOldGenerationSizeMb: -1,
    codeRangeSizeMb: -1,
    stackSizeMb: 4,
  };

  constructor(specifier: URL | string, options?: WorkerOptions) {
    super();

    if (
      typeof specifier === "object" &&
      !(specifier.protocol === "data:" || specifier.protocol === "file:")
    ) {
      throw new TypeError(
        "node:worker_threads support only 'file:' and 'data:' URLs",
      );
    }
    if (options?.eval) {
      specifier = `data:text/javascript,${specifier}`;
    } else if (
      !(typeof specifier === "object" && specifier.protocol === "data:")
    ) {
      // deno-lint-ignore prefer-primordials
      specifier = specifier.toString();
      specifier = op_worker_threads_filename(specifier);
    }

    // TODO(bartlomieu): this doesn't match the Node.js behavior, it should be
    // `[worker {threadId}] {name}` or empty string.
    let name = StringPrototypeTrim(options?.name ?? "");
    if (options?.eval) {
      name = "[worker eval]";
    }
    this.#name = name;

    // One of the most common usages will be to pass `process.env` here,
    // but because `process.env` is a Proxy in Deno, we need to get a plain
    // object out of it - otherwise we'll run in `DataCloneError`s.
    // See https://github.com/denoland/deno/issues/23522.
    let env_ = undefined;
    if (options?.env) {
      env_ = JSONParse(JSONStringify(options?.env));
    }
    const serializedWorkerMetadata = serializeJsMessageData({
      workerData: options?.workerData,
      environmentData: environmentData,
      env: env_,
      isWorkerThread: true,
    }, options?.transferList ?? []);
    const id = op_create_worker(
      {
        // deno-lint-ignore prefer-primordials
        specifier: specifier.toString(),
        hasSourceCode: false,
        sourceCode: "",
        permissions: null,
        name: this.#name,
        workerType: "module",
        closeOnIdle: true,
      },
      serializedWorkerMetadata,
    );
    this.#id = id;
    this.threadId = id;
    this.#pollControl();
    this.#pollMessages();
  }

  [privateWorkerRef](ref) {
    if (ref) {
      this.#refCount++;
    } else {
      this.#refCount--;
    }

    if (!ref && this.#refCount == 0) {
      if (this.#controlPromise) {
        core.unrefOpPromise(this.#controlPromise);
      }
      if (this.#messagePromise) {
        core.unrefOpPromise(this.#messagePromise);
      }
    } else if (ref && this.#refCount == 1) {
      if (this.#controlPromise) {
        core.refOpPromise(this.#controlPromise);
      }
      if (this.#messagePromise) {
        core.refOpPromise(this.#messagePromise);
      }
    }
  }

  #handleError(err) {
    this.emit("error", err);
  }

  #pollControl = async () => {
    while (this.#status === "RUNNING") {
      this.#controlPromise = op_host_recv_ctrl(this.#id);
      if (this.#refCount < 1) {
        core.unrefOpPromise(this.#controlPromise);
      }
      const { 0: type, 1: data } = await this.#controlPromise;

      // If terminate was called then we ignore all messages
      if (this.#status === "TERMINATED") {
        return;
      }

      switch (type) {
        case 1: { // TerminalError
          this.#status = "CLOSED";
        } /* falls through */
        case 2: { // Error
          this.#handleError(data);
          break;
        }
        case 3: { // Close
          debugWT(`Host got "close" message from worker: ${this.#name}`);
          this.#status = "CLOSED";
          return;
        }
        default: {
          throw new Error(`Unknown worker event: "${type}"`);
        }
      }
    }
  };

  #pollMessages = async () => {
    while (this.#status !== "TERMINATED") {
      this.#messagePromise = op_host_recv_message(this.#id);
      if (this.#refCount < 1) {
        core.unrefOpPromise(this.#messagePromise);
      }
      const data = await this.#messagePromise;
      if (this.#status === "TERMINATED" || data === null) {
        return;
      }
      let message, _transferables;
      try {
        const v = deserializeJsMessageData(data);
        message = v[0];
        _transferables = v[1];
      } catch (err) {
        this.emit("messageerror", err);
        return;
      }
      if (
        // only emit "online" event once, and since the message
        // has to come before user messages, we are safe to assume
        // it came from us
        !this.#workerOnline && isWorkerOnlineMsg(message)
      ) {
        this.#workerOnline = true;
        this.emit("online");
      } else {
        this.emit("message", message);
      }
    }
  };

  postMessage(message, transferOrOptions = { __proto__: null }) {
    const prefix = "Failed to execute 'postMessage' on 'MessagePort'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    message = webidl.converters.any(message);
    let options;
    if (
      webidl.type(transferOrOptions) === "Object" &&
      transferOrOptions !== undefined &&
      transferOrOptions[SymbolIterator] !== undefined
    ) {
      const transfer = webidl.converters["sequence<object>"](
        transferOrOptions,
        prefix,
        "Argument 2",
      );
      options = { transfer };
    } else {
      options = webidl.converters.StructuredSerializeOptions(
        transferOrOptions,
        prefix,
        "Argument 2",
      );
    }
    const { transfer } = options;
    const data = serializeJsMessageData(message, transfer);
    if (this.#status === "RUNNING") {
      op_host_post_message(this.#id, data);
    }
  }

  // https://nodejs.org/api/worker_threads.html#workerterminate
  terminate() {
    if (this.#status !== "TERMINATED") {
      this.#status = "TERMINATED";
      op_host_terminate_worker(this.#id);
      this.emit("exit", 0);
    }
    return PromiseResolve(0);
  }

  ref() {
    this[privateWorkerRef](true);
  }

  unref() {
    this[privateWorkerRef](false);
  }

  readonly getHeapSnapshot = () =>
    notImplemented("Worker.prototype.getHeapSnapshot");
  // fake performance
  readonly performance = globalThis.performance;
}

export let isMainThread;
export let resourceLimits;

let threadId = 0;
let workerData: unknown = null;
let environmentData = new SafeMap();

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
let parentPort: ParentPort = null as any;

internals.__initWorkerThreads = (
  runningOnMainThread: boolean,
  workerId,
  maybeWorkerMetadata,
) => {
  isMainThread = runningOnMainThread;

  defaultExport.isMainThread = isMainThread;
  // fake resourceLimits
  resourceLimits = isMainThread ? {} : {
    maxYoungGenerationSizeMb: 48,
    maxOldGenerationSizeMb: 2048,
    codeRangeSizeMb: 0,
    stackSizeMb: 4,
  };
  defaultExport.resourceLimits = resourceLimits;

  if (!isMainThread) {
    const listeners = new SafeWeakMap<
      // deno-lint-ignore no-explicit-any
      (...args: any[]) => void,
      // deno-lint-ignore no-explicit-any
      (ev: any) => any
    >();

    parentPort = globalThis as ParentPort;
    threadId = workerId;
    let isWorkerThread = false;
    if (maybeWorkerMetadata) {
      const { 0: metadata, 1: _ } = maybeWorkerMetadata;
      workerData = metadata.workerData;
      environmentData = metadata.environmentData;
      isWorkerThread = metadata.isWorkerThread;
      const env = metadata.env;
      if (env) {
        process.env = env;
      }
    }
    defaultExport.workerData = workerData;
    defaultExport.parentPort = parentPort;
    defaultExport.threadId = threadId;

    patchMessagePortIfFound(workerData);

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
      const _listener = (ev: any) => {
        const message = ev.data;
        patchMessagePortIfFound(message);
        return listener(message);
      };
      listeners.set(listener, _listener);
      this.addEventListener(name, _listener);
      return this;
    };

    parentPort.once = function (this: ParentPort, name, listener) {
      // deno-lint-ignore no-explicit-any
      const _listener = (ev: any) => {
        const message = ev.data;
        patchMessagePortIfFound(message);
        return listener(message);
      };
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

    parentPort.addEventListener("offline", () => {
      parentPort.emit("close");
    });
    parentPort.unref = () => {
      parentPort[unrefPollForMessages] = true;
    };
    parentPort.ref = () => {
      parentPort[unrefPollForMessages] = false;
    };

    if (isWorkerThread) {
      // Notify the host that the worker is online
      parentPort.postMessage(
        {
          type: "WORKER_ONLINE",
        } satisfies WorkerOnlineMsg,
      );
    }
  }
};

export function getEnvironmentData(key: unknown) {
  return environmentData.get(key);
}

export function setEnvironmentData(key: unknown, value?: unknown) {
  if (value === undefined) {
    environmentData.delete(key);
  } else {
    environmentData.set(key, value);
  }
}

export const SHARE_ENV = SymbolFor("nodejs.worker_threads.SHARE_ENV");
export function markAsUntransferable() {
  notImplemented("markAsUntransferable");
}
export function moveMessagePortToContext() {
  notImplemented("moveMessagePortToContext");
}

/**
 * @param { MessagePort } port
 * @returns {object | undefined}
 */
export function receiveMessageOnPort(port: MessagePort): object | undefined {
  if (!(ObjectPrototypeIsPrototypeOf(MessagePortPrototype, port))) {
    const err = new TypeError(
      'The "port" argument must be a MessagePort instance',
    );
    err["code"] = "ERR_INVALID_ARG_TYPE";
    throw err;
  }
  port[MessagePortReceiveMessageOnPortSymbol] = true;
  const data = op_message_port_recv_message_sync(port[MessagePortIdSymbol]);
  if (data === null) return undefined;
  const message = deserializeJsMessageData(data)[0];
  patchMessagePortIfFound(message);
  return { message };
}

class NodeMessageChannel {
  port1: MessagePort;
  port2: MessagePort;

  constructor() {
    const { port1, port2 } = new MessageChannel();
    this.port1 = webMessagePortToNodeMessagePort(port1);
    this.port2 = webMessagePortToNodeMessagePort(port2);
  }
}

const listeners = new SafeWeakMap<
  // deno-lint-ignore no-explicit-any
  (...args: any[]) => void,
  // deno-lint-ignore no-explicit-any
  (ev: any) => any
>();
function webMessagePortToNodeMessagePort(port: MessagePort) {
  port.on = port.addListener = function (this: MessagePort, name, listener) {
    // deno-lint-ignore no-explicit-any
    const _listener = (ev: any) => {
      patchMessagePortIfFound(ev.data);
      listener(ev.data);
    };
    if (name == "message") {
      if (port.onmessage === null) {
        port.onmessage = _listener;
      } else {
        port.addEventListener("message", _listener);
      }
    } else if (name == "messageerror") {
      if (port.onmessageerror === null) {
        port.onmessageerror = _listener;
      } else {
        port.addEventListener("messageerror", _listener);
      }
    } else if (name == "close") {
      port.addEventListener("close", _listener);
    } else {
      throw new Error(`Unknown event: "${name}"`);
    }
    listeners.set(listener, _listener);
    return this;
  };
  port.off = port.removeListener = function (
    this: MessagePort,
    name,
    listener,
  ) {
    if (name == "message") {
      port.removeEventListener("message", listeners.get(listener)!);
    } else if (name == "messageerror") {
      port.removeEventListener("messageerror", listeners.get(listener)!);
    } else if (name == "close") {
      port.removeEventListener("close", listeners.get(listener)!);
    } else {
      throw new Error(`Unknown event: "${name}"`);
    }
    listeners.delete(listener);
    return this;
  };
  port[nodeWorkerThreadCloseCb] = () => {
    port.dispatchEvent(new Event("close"));
  };
  port.unref = () => {
    port[refMessagePort](false);
  };
  port.ref = () => {
    port[refMessagePort](true);
  };
  port.once = (name: string | symbol, listener) => {
    const fn = (event) => {
      port.off(name, fn);
      return listener(event);
    };
    port.on(name, fn);
  };
  return port;
}

// TODO(@marvinhagemeister): Recursively iterating over all message
// properties seems slow.
// Maybe there is a way we can patch the prototype of MessagePort _only_
// inside worker_threads? For now correctness is more important than perf.
// deno-lint-ignore no-explicit-any
function patchMessagePortIfFound(data: any, seen = new SafeSet<any>()) {
  if (data === null || typeof data !== "object" || seen.has(data)) {
    return;
  }
  seen.add(data);

  if (ObjectPrototypeIsPrototypeOf(MessagePortPrototype, data)) {
    webMessagePortToNodeMessagePort(data);
  } else {
    for (const obj in data as Record<string, unknown>) {
      if (ObjectHasOwn(data, obj)) {
        patchMessagePortIfFound(data[obj], seen);
      }
    }
  }
}

export {
  BroadcastChannel,
  MessagePort,
  NodeMessageChannel as MessageChannel,
  NodeWorker as Worker,
  parentPort,
  threadId,
  workerData,
};

const defaultExport = {
  markAsUntransferable,
  moveMessagePortToContext,
  receiveMessageOnPort,
  MessagePort,
  MessageChannel: NodeMessageChannel,
  BroadcastChannel,
  Worker: NodeWorker,
  getEnvironmentData,
  setEnvironmentData,
  SHARE_ENV,
  threadId,
  workerData,
  resourceLimits,
  parentPort,
  isMainThread,
};

export default defaultExport;
