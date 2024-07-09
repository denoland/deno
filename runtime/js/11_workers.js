// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
import {
  op_create_worker,
  op_host_post_message,
  op_host_recv_ctrl,
  op_host_recv_message,
  op_host_terminate_worker,
} from "ext:core/ops";
const {
  ArrayPrototypeFilter,
  Error,
  ObjectPrototypeIsPrototypeOf,
  String,
  StringPrototypeStartsWith,
  Symbol,
  SymbolFor,
  SymbolIterator,
  SymbolToStringTag,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import { URL } from "ext:deno_url/00_url.js";
import { getLocationHref } from "ext:deno_web/12_location.js";
import { serializePermissions } from "ext:runtime/10_permissions.js";
import { log } from "ext:runtime/06_util.js";
import {
  defineEventHandler,
  ErrorEvent,
  EventTarget,
  MessageEvent,
  setIsTrusted,
} from "ext:deno_web/02_event.js";
import {
  deserializeJsMessageData,
  MessagePortPrototype,
  serializeJsMessageData,
} from "ext:deno_web/13_message_port.js";

function createWorker(
  specifier,
  hasSourceCode,
  sourceCode,
  permissions,
  name,
  workerType,
  closeOnIdle,
) {
  return op_create_worker({
    hasSourceCode,
    name,
    permissions: serializePermissions(permissions),
    sourceCode,
    specifier,
    workerType,
    closeOnIdle,
  });
}

function hostTerminateWorker(id) {
  op_host_terminate_worker(id);
}

function hostPostMessage(id, data) {
  op_host_post_message(id, data);
}

function hostRecvCtrl(id) {
  return op_host_recv_ctrl(id);
}

function hostRecvMessage(id) {
  return op_host_recv_message(id);
}

const privateWorkerRef = Symbol();

class Worker extends EventTarget {
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

  constructor(specifier, options = { __proto__: null }) {
    super();
    specifier = String(specifier);
    const {
      deno,
      name,
      type = "classic",
    } = options;

    const workerType = webidl.converters["WorkerType"](type);

    if (
      StringPrototypeStartsWith(specifier, "./") ||
      StringPrototypeStartsWith(specifier, "../") ||
      StringPrototypeStartsWith(specifier, "/") || workerType === "classic"
    ) {
      const baseUrl = getLocationHref();
      if (baseUrl != null) {
        specifier = new URL(specifier, baseUrl).href;
      }
    }

    this.#name = name;
    let hasSourceCode, sourceCode;
    if (workerType === "classic") {
      hasSourceCode = true;
      sourceCode = `importScripts("#");`;
    } else {
      hasSourceCode = false;
      sourceCode = "";
    }

    const id = createWorker(
      specifier,
      hasSourceCode,
      sourceCode,
      deno?.permissions,
      this.#name,
      workerType,
      false,
    );
    this.#id = id;
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

  #handleError(e) {
    const event = new ErrorEvent("error", {
      cancelable: true,
      message: e.message,
      lineno: e.lineNumber ? e.lineNumber : undefined,
      colno: e.columnNumber ? e.columnNumber : undefined,
      filename: e.fileName,
      error: null,
    });

    this.dispatchEvent(event);
    // Don't bubble error event to window for loader errors (`!e.fileName`).
    // TODO(nayeemrmn): It's not correct to use `e.fileName` to detect user
    // errors. It won't be there for non-awaited async ops for example.
    if (e.fileName && !event.defaultPrevented) {
      globalThis.dispatchEvent(event);
    }

    return event.defaultPrevented;
  }

  #pollControl = async () => {
    while (this.#status === "RUNNING") {
      this.#controlPromise = hostRecvCtrl(this.#id);
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
          if (!this.#handleError(data)) {
            throw new Error("Unhandled error in child worker.");
          }
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
      this.#messagePromise = hostRecvMessage(this.#id);
      if (this.#refCount < 1) {
        core.unrefOpPromise(this.#messagePromise);
      }
      const data = await this.#messagePromise;
      if (this.#status === "TERMINATED" || data === null) {
        return;
      }
      let message, transferables;
      try {
        const v = deserializeJsMessageData(data);
        message = v[0];
        transferables = v[1];
      } catch (err) {
        const event = new MessageEvent("messageerror", {
          cancelable: false,
          data: err,
        });
        setIsTrusted(event, true);
        this.dispatchEvent(event);
        return;
      }
      const event = new MessageEvent("message", {
        cancelable: false,
        data: message,
        ports: ArrayPrototypeFilter(
          transferables,
          (t) => ObjectPrototypeIsPrototypeOf(MessagePortPrototype, t),
        ),
      });
      setIsTrusted(event, true);
      this.dispatchEvent(event);
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
      hostPostMessage(this.#id, data);
    }
  }

  terminate() {
    if (this.#status !== "TERMINATED") {
      this.#status = "TERMINATED";
      hostTerminateWorker(this.#id);
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(WorkerPrototype, this),
        keys: [
          "onerror",
          "onmessage",
          "onmessageerror",
        ],
      }),
      inspectOptions,
    );
  }

  [SymbolToStringTag] = "Worker";
}

const WorkerPrototype = Worker.prototype;

defineEventHandler(Worker.prototype, "error");
defineEventHandler(Worker.prototype, "message");
defineEventHandler(Worker.prototype, "messageerror");

webidl.converters["WorkerType"] = webidl.createEnumConverter("WorkerType", [
  "classic",
  "module",
]);

export { Worker };
