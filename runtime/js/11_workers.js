// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const {
    Error,
    ObjectPrototypeIsPrototypeOf,
    StringPrototypeStartsWith,
    String,
    SymbolIterator,
    SymbolToStringTag,
  } = window.__bootstrap.primordials;
  const webidl = window.__bootstrap.webidl;
  const { URL } = window.__bootstrap.url;
  const { getLocationHref } = window.__bootstrap.location;
  const { serializePermissions } = window.__bootstrap.permissions;
  const { log } = window.__bootstrap.util;
  const { defineEventHandler } = window.__bootstrap.event;
  const {
    deserializeJsMessageData,
    serializeJsMessageData,
    MessagePortPrototype,
  } = window.__bootstrap.messagePort;

  function createWorker(
    specifier,
    hasSourceCode,
    sourceCode,
    permissions,
    name,
    workerType,
  ) {
    return ops.op_create_worker({
      hasSourceCode,
      name,
      permissions: serializePermissions(permissions),
      sourceCode,
      specifier,
      workerType,
    });
  }

  function hostTerminateWorker(id) {
    ops.op_host_terminate_worker(id);
  }

  function hostPostMessage(id, data) {
    ops.op_host_post_message(id, data);
  }

  function hostRecvCtrl(id) {
    return core.opAsync("op_host_recv_ctrl", id);
  }

  function hostRecvMessage(id) {
    return core.opAsync("op_host_recv_message", id);
  }

  class Worker extends EventTarget {
    #id = 0;
    #name = "";

    // "RUNNING" | "CLOSED" | "TERMINATED"
    // "TERMINATED" means that any controls or messages received will be
    // discarded. "CLOSED" means that we have received a control
    // indicating that the worker is no longer running, but there might
    // still be messages left to receive.
    #status = "RUNNING";

    constructor(specifier, options = {}) {
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
        name,
        workerType,
      );
      this.#id = id;
      this.#pollControl();
      this.#pollMessages();
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
        window.dispatchEvent(event);
      }

      return event.defaultPrevented;
    }

    #pollControl = async () => {
      while (this.#status === "RUNNING") {
        const [type, data] = await hostRecvCtrl(this.#id);

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
        const data = await hostRecvMessage(this.#id);
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
          this.dispatchEvent(event);
          return;
        }
        const event = new MessageEvent("message", {
          cancelable: false,
          data: message,
          ports: transferables.filter((t) =>
            ObjectPrototypeIsPrototypeOf(MessagePortPrototype, t)
          ),
        });
        this.dispatchEvent(event);
      }
    };

    postMessage(message, transferOrOptions = {}) {
      const prefix = "Failed to execute 'postMessage' on 'MessagePort'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      message = webidl.converters.any(message);
      let options;
      if (
        webidl.type(transferOrOptions) === "Object" &&
        transferOrOptions !== undefined &&
        transferOrOptions[SymbolIterator] !== undefined
      ) {
        const transfer = webidl.converters["sequence<object>"](
          transferOrOptions,
          { prefix, context: "Argument 2" },
        );
        options = { transfer };
      } else {
        options = webidl.converters.StructuredSerializeOptions(
          transferOrOptions,
          {
            prefix,
            context: "Argument 2",
          },
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

    [SymbolToStringTag] = "Worker";
  }

  defineEventHandler(Worker.prototype, "error");
  defineEventHandler(Worker.prototype, "message");
  defineEventHandler(Worker.prototype, "messageerror");

  webidl.converters["WorkerType"] = webidl.createEnumConverter("WorkerType", [
    "classic",
    "module",
  ]);

  window.__bootstrap.worker = {
    Worker,
  };
})(this);
