// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const {
    ArrayIsArray,
    ArrayPrototypeIncludes,
    ArrayPrototypeMap,
    Error,
    StringPrototypeStartsWith,
    String,
    SymbolIterator,
  } = window.__bootstrap.primordials;
  const webidl = window.__bootstrap.webidl;
  const { URL } = window.__bootstrap.url;
  const { Window } = window.__bootstrap.globalInterfaces;
  const { getLocationHref } = window.__bootstrap.location;
  const { log, pathFromURL } = window.__bootstrap.util;
  const { defineEventHandler } = window.__bootstrap.webUtil;
  const { deserializeJsMessageData, serializeJsMessageData } =
    window.__bootstrap.messagePort;

  function createWorker(
    specifier,
    hasSourceCode,
    sourceCode,
    permissions,
    name,
    workerType,
  ) {
    return core.opSync("op_create_worker", {
      hasSourceCode,
      name,
      permissions,
      sourceCode,
      specifier,
      workerType,
    });
  }

  function hostTerminateWorker(id) {
    core.opSync("op_host_terminate_worker", id);
  }

  function hostPostMessage(id, data) {
    core.opSync("op_host_post_message", id, data);
  }

  function hostRecvCtrl(id) {
    return core.opAsync("op_host_recv_ctrl", id);
  }

  function hostRecvMessage(id) {
    return core.opAsync("op_host_recv_message", id);
  }

  /**
   * @return {boolean}
   */
  function normalizeUnitPermission(value) {
    return value === "inherit" ? undefined : value;
  }

  /**
   * @param {string} permName
   * @return {(boolean | string[])}
   */
  function normalizeUnaryPermission(value, permName) {
    if (value === "inherit") {
      return undefined;
    } else if (ArrayIsArray(value)) {
      return ArrayPrototypeMap(value, (route) => {
        if (route instanceof URL) {
          if (ArrayPrototypeIncludes(["read", "write", "run"], permName)) {
            route = pathFromURL(route);
          }
        }
        return route;
      });
    } else {
      return value;
    }
  }

  /**
   * Normalizes permissions options for deserializing in Rust:
   * - Changes `"none"` to `{ <all perms>: false }`.
   * - Changes any `"inherit"` to `undefined`.
   * - Converts all file URLs in FS allowlists to paths.
   */
  function normalizePermissions(permissions) {
    if (permissions == null || permissions === "inherit") {
      return undefined;
    } else if (permissions === "none") {
      return {
        env: false,
        hrtime: false,
        net: false,
        ffi: false,
        read: false,
        run: false,
        write: false,
      };
    } else if (typeof permissions == "object") {
      return {
        env: normalizeUnitPermission(permissions.env ?? "inherit"),
        hrtime: normalizeUnitPermission(permissions.hrtime ?? "inherit"),
        net: normalizeUnaryPermission(permissions.net ?? "inherit", "net"),
        ffi: normalizeUnitPermission(permissions.ffi ?? "inherit"),
        read: normalizeUnaryPermission(permissions.read ?? "inherit", "read"),
        run: normalizeUnaryPermission(permissions.run ?? "inherit", "run"),
        write: normalizeUnaryPermission(
          permissions.write ?? "inherit",
          "write",
        ),
      };
    } else {
      // This should be a deserializing error in Rust.
      return permissions;
    }
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
        deno: {
          permissions = "inherit",
        } = {},
        name = "unknown",
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
        normalizePermissions(permissions),
        options?.name,
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
        lineno: e.lineNumber ? e.lineNumber + 1 : undefined,
        colno: e.columnNumber ? e.columnNumber + 1 : undefined,
        filename: e.fileName,
        error: null,
      });

      let handled = false;

      this.dispatchEvent(event);
      if (event.defaultPrevented) {
        handled = true;
      }

      return handled;
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
              if (globalThis instanceof Window) {
                throw new Error("Unhandled error event reached main worker.");
              } else {
                core.opSync(
                  "op_worker_unhandled_error",
                  data.message,
                );
              }
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
          ports: transferables.filter((t) => t instanceof MessagePort),
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
  }

  defineEventHandler(Worker.prototype, "error");
  defineEventHandler(Worker.prototype, "message");
  defineEventHandler(Worker.prototype, "messageerror");

  webidl.converters["WorkerType"] = webidl.createEnumConverter("WorkerType", [
    "classic",
    "module",
  ]);

  window.__bootstrap.worker = {
    normalizePermissions,
    Worker,
  };
})(this);
