// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { core, internals, primordials } from "ext:core/mod.js";
import {
  op_create_worker,
  op_host_post_message,
  op_host_recv_ctrl,
  op_host_recv_message,
  op_host_terminate_worker,
  op_require_read_closest_package_json,
} from "ext:core/ops";
import { BroadcastChannel } from "ext:deno_broadcast_channel/01_broadcast_channel.js";
import {
  deserializeJsMessageData,
  MessageChannel,
  MessagePort,
  serializeJsMessageData,
} from "ext:deno_web/13_message_port.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { log } from "ext:runtime/06_util.js";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { EventEmitter, once } from "node:events";
import { isAbsolute, resolve } from "node:path";

const {
  Error,
  Symbol,
  SymbolFor,
  SymbolIterator,
  StringPrototypeEndsWith,
  StringPrototypeReplace,
  StringPrototypeMatch,
  StringPrototypeReplaceAll,
  StringPrototypeToString,
  StringPrototypeTrim,
  SafeWeakMap,
  SafeRegExp,
  SafeMap,
  TypeError,
  PromisePrototypeThen,
} = primordials;

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

const WHITESPACE_ENCODINGS: Record<string, string> = {
  "\u0009": "%09",
  "\u000A": "%0A",
  "\u000B": "%0B",
  "\u000C": "%0C",
  "\u000D": "%0D",
  "\u0020": "%20",
};

function encodeWhitespace(string: string): string {
  return StringPrototypeReplaceAll(string, new SafeRegExp(/[\s]/g), (c) => {
    return WHITESPACE_ENCODINGS[c] ?? c;
  });
}

function toFileUrlPosix(path: string): URL {
  if (!isAbsolute(path)) {
    throw new TypeError("Must be an absolute path.");
  }
  const url = new URL("file:///");
  url.pathname = encodeWhitespace(
    StringPrototypeReplace(
      StringPrototypeReplace(path, new SafeRegExp(/%/g), "%25"),
      new SafeRegExp(/\\/g),
      "%5C",
    ),
  );
  return url;
}

function toFileUrlWin32(path: string): URL {
  if (!isAbsolute(path)) {
    throw new TypeError("Must be an absolute path.");
  }
  const { 0: _, 1: hostname, 2: pathname } = StringPrototypeMatch(
    path,
    new SafeRegExp(/^(?:[/\\]{2}([^/\\]+)(?=[/\\](?:[^/\\]|$)))?(.*)/),
  );
  const url = new URL("file:///");
  url.pathname = encodeWhitespace(
    StringPrototypeReplace(pathname, new SafeRegExp(/%/g), "%25"),
  );
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

let threads = 0;
const privateWorkerRef = Symbol("privateWorkerRef");
class NodeWorker extends EventEmitter {
  #id = 0;
  #name = "";
  #refCount = 1;
  #messagePromise = undefined;
  #controlPromise = undefined;
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
    if (options?.eval === true) {
      specifier = `data:text/javascript,${specifier}`;
    } else if (typeof specifier === "string") {
      specifier = resolve(specifier);
      let pkg;
      try {
        pkg = op_require_read_closest_package_json(specifier);
      } catch (_) {
        // empty catch block when package json might not be present
      }
      if (
        !(StringPrototypeEndsWith(
          StringPrototypeToString(specifier),
          ".mjs",
        )) ||
        (pkg && pkg.exists && pkg.typ == "module")
      ) {
        const cwdFileUrl = toFileUrl(Deno.cwd());
        specifier =
          `data:text/javascript,(async function() {const { createRequire } = await import("node:module");const require = createRequire("${cwdFileUrl}");require("${specifier}");})();`;
      } else {
        specifier = toFileUrl(specifier as string);
      }
    }

    // TODO(bartlomieu): this doesn't match the Node.js behavior, it should be
    // `[worker {threadId}] {name}` or empty string.
    let name = StringPrototypeTrim(options?.name ?? "");
    if (options?.eval) {
      name = "[worker eval]";
    }
    this.#name = name;
    const id = op_create_worker(
      {
        // deno-lint-ignore prefer-primordials
        specifier: specifier.toString(),
        hasSourceCode: false,
        sourceCode: "",
        permissions: null,
        name: this.#name,
        workerType: "module",
      },
    );
    this.#id = id;
    this.#pollControl();
    this.#pollMessages();

    this.postMessage({
      environmentData,
      threadId: (this.threadId = ++threads),
      workerData: options?.workerData,
    }, options?.transferList || []);
    // https://nodejs.org/api/worker_threads.html#event-online
    this.emit("online");
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
          log(`Host got "close" message from worker: ${this.#name}`);
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
      this.emit("message", message);
    }
  };

  postMessage(message, transferOrOptions = {}) {
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
    }
    this.emit("exit", 1);
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

internals.__initWorkerThreads = (runningOnMainThread: boolean) => {
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

    parentPort = self as ParentPort;

    const initPromise = PromisePrototypeThen(
      once(
        parentPort,
        "message",
      ),
      (result) => {
        // TODO(bartlomieju): just so we don't error out here. It's still racy,
        // but should be addressed by https://github.com/denoland/deno/issues/22783
        // shortly.
        const data = result[0].data ?? {};
        // TODO(kt3k): The below values are set asynchronously
        // using the first message from the parent.
        // This should be done synchronously.
        threadId = data.threadId;
        workerData = data.workerData;
        environmentData = data.environmentData;

        defaultExport.threadId = threadId;
        defaultExport.workerData = workerData;
      },
    );

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
      PromisePrototypeThen(initPromise, () => {
        // deno-lint-ignore no-explicit-any
        const _listener = (ev: any) => listener(ev.data);
        listeners.set(listener, _listener);
        this.addEventListener(name, _listener);
      });
      return this;
    };

    parentPort.once = function (this: ParentPort, name, listener) {
      PromisePrototypeThen(initPromise, () => {
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

export const SHARE_ENV = SymbolFor("nodejs.worker_threads.SHARE_ENV");
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
  BroadcastChannel,
  MessageChannel,
  MessagePort,
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
  MessageChannel,
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
