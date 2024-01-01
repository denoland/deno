// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { isAbsolute, resolve } from "node:path";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { EventEmitter, once } from "node:events";
import { BroadcastChannel } from "ext:deno_broadcast_channel/01_broadcast_channel.js";
import { MessageChannel, MessagePort } from "ext:deno_web/13_message_port.js";

let environmentData = new Map();
let threads = 0;
const { core } = globalThis.__bootstrap;

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

const WHITESPACE_ENCODINGS: Record<string, string> = {
  "\u0009": "%09",
  "\u000A": "%0A",
  "\u000B": "%0B",
  "\u000C": "%0C",
  "\u000D": "%0D",
  "\u0020": "%20",
};

function encodeWhitespace(string: string): string {
  return string.replaceAll(/[\s]/g, (c) => {
    return WHITESPACE_ENCODINGS[c] ?? c;
  });
}

function toFileUrlPosix(path: string): URL {
  if (!isAbsolute(path)) {
    throw new TypeError("Must be an absolute path.");
  }
  const url = new URL("file:///");
  url.pathname = encodeWhitespace(
    path.replace(/%/g, "%25").replace(/\\/g, "%5C"),
  );
  return url;
}

function toFileUrlWin32(path: string): URL {
  if (!isAbsolute(path)) {
    throw new TypeError("Must be an absolute path.");
  }
  const [, hostname, pathname] = path.match(
    /^(?:[/\\]{2}([^/\\]+)(?=[/\\](?:[^/\\]|$)))?(.*)/,
  )!;
  const url = new URL("file:///");
  url.pathname = encodeWhitespace(pathname.replace(/%/g, "%25"));
  if (hostname != null && hostname != "localhost") {
    url.hostname = hostname;
    if (!url.hostname) {
      throw new TypeError("Invalid hostname.");
    }
  }
  return url;
}

/**
 * Converts a path string to a file URL.
 *
 * ```ts
 *      toFileUrl("/home/foo"); // new URL("file:///home/foo")
 *      toFileUrl("\\home\\foo"); // new URL("file:///home/foo")
 *      toFileUrl("C:\\Users\\foo"); // new URL("file:///C:/Users/foo")
 *      toFileUrl("\\\\127.0.0.1\\home\\foo"); // new URL("file://127.0.0.1/home/foo")
 * ```
 * @param path to convert to file URL
 */
function toFileUrl(path: string): URL {
  return core.build.os == "windows"
    ? toFileUrlWin32(path)
    : toFileUrlPosix(path);
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
    super();
    if (options?.eval === true) {
      specifier = `data:text/javascript,${specifier}`;
    } else if (typeof specifier === "string") {
      specifier = resolve(specifier);
      let pkg;
      try {
        pkg = core.ops.op_require_read_closest_package_json(specifier);
      } catch (_) {
        // empty catch block when package json might not be present
      }
      if (
        !(specifier.toString().endsWith(".mjs") ||
          (pkg && pkg.exists && pkg.typ == "module"))
      ) {
        const cwdFileUrl = toFileUrl(Deno.cwd());
        specifier =
          `data:text/javascript,(async function() {const { createRequire } = await import("node:module");const require = createRequire("${cwdFileUrl}");require("${specifier}");})();`;
      } else {
        specifier = toFileUrl(specifier as string);
      }
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

export let isMainThread;
export let resourceLimits;

let threadId = 0;
let workerData: unknown = null;

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

globalThis.__bootstrap.internals.__initWorkerThreads = () => {
  isMainThread =
    // deno-lint-ignore no-explicit-any
    (globalThis as any).name !== PRIVATE_WORKER_THREAD_NAME;

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
    // deno-lint-ignore no-explicit-any
    delete (globalThis as any).name;
    // deno-lint-ignore no-explicit-any
    const listeners = new WeakMap<(...args: any[]) => void, (ev: any) => any>();

    parentPort = self as ParentPort;

    const initPromise = once(
      parentPort,
      "message",
    ).then((result) => {
      // TODO(kt3k): The below values are set asynchronously
      // using the first message from the parent.
      // This should be done synchronously.
      threadId = result[0].data.threadId;
      workerData = result[0].data.workerData;
      environmentData = result[0].data.environmentData;

      defaultExport.threadId = threadId;
      defaultExport.workerData = workerData;
    });

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
      initPromise.then(() => {
        // deno-lint-ignore no-explicit-any
        const _listener = (ev: any) => listener(ev.data);
        listeners.set(listener, _listener);
        this.addEventListener(name, _listener);
      });
      return this;
    };

    parentPort.once = function (this: ParentPort, name, listener) {
      initPromise.then(() => {
        // deno-lint-ignore no-explicit-any
        const _listener = (ev: any) => listener(ev.data);
        listeners.set(listener, _listener);
        this.addEventListener(name, _listener);
      });
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
  _Worker as Worker,
  BroadcastChannel,
  MessageChannel,
  MessagePort,
  parentPort,
  threadId,
  workerData,
};

const defaultExport = {
  markAsUntransferable,
  moveMessagePortToContext,
  receiveMessageOnPort,
  MessagePort,
  MessageChannel,
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

export default defaultExport;
