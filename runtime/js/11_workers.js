// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const {
    ArrayIsArray,
    ArrayPrototypeMap,
    Error,
    Uint8Array,
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
    useDenoNamespace,
    permissions,
    name,
  ) {
    return core.opSync("op_create_worker", {
      hasSourceCode,
      name,
      permissions,
      sourceCode,
      specifier,
      useDenoNamespace,
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
   * @param {string} permission
   * @return {boolean}
   */
  function parseUnitPermission(
    value,
    permission,
  ) {
    if (value !== "inherit" && typeof value !== "boolean") {
      throw new Error(
        `Expected 'boolean' for ${permission} permission, ${typeof value} received`,
      );
    }
    return value === "inherit" ? undefined : value;
  }

  /**
   * @param {string} permission
   * @return {(boolean | string[])}
   * */
  function parseArrayPermission(
    value,
    permission,
  ) {
    if (typeof value === "string") {
      if (value !== "inherit") {
        throw new Error(
          `Expected 'array' or 'boolean' for ${permission} permission, "${value}" received`,
        );
      }
    } else if (!ArrayIsArray(value) && typeof value !== "boolean") {
      throw new Error(
        `Expected 'array' or 'boolean' for ${permission} permission, ${typeof value} received`,
      );
      //Casts URLs to absolute routes
    } else if (ArrayIsArray(value)) {
      value = ArrayPrototypeMap(value, (route) => {
        if (route instanceof URL) {
          route = pathFromURL(route);
        }
        return route;
      });
    }

    return value === "inherit" ? undefined : value;
  }

  /**
   * Normalizes data, runs checks on parameters and deletes inherited permissions
   */
  function parsePermissions({
    env = "inherit",
    hrtime = "inherit",
    net = "inherit",
    plugin = "inherit",
    read = "inherit",
    run = "inherit",
    write = "inherit",
  }) {
    return {
      env: parseUnitPermission(env, "env"),
      hrtime: parseUnitPermission(hrtime, "hrtime"),
      net: parseArrayPermission(net, "net"),
      plugin: parseUnitPermission(plugin, "plugin"),
      read: parseArrayPermission(read, "read"),
      run: parseUnitPermission(run, "run"),
      write: parseArrayPermission(write, "write"),
    };
  }

  class Worker extends EventTarget {
    #id = 0;
    #name = "";
    #terminated = false;

    constructor(specifier, options = {}) {
      super();
      specifier = String(specifier);
      const {
        deno = {},
        name = "unknown",
        type = "classic",
      } = options;

      // TODO(Soremwar)
      // `deno: boolean` is kept for backwards compatibility with the previous
      // worker options implementation. Remove for 2.0
      let workerDenoAttributes;
      if (typeof deno == "boolean") {
        workerDenoAttributes = {
          // Change this to enable the Deno namespace by default
          namespace: deno,
          permissions: null,
        };
      } else {
        workerDenoAttributes = {
          // Change this to enable the Deno namespace by default
          namespace: !!(deno?.namespace ?? false),
          permissions: (deno?.permissions ?? "inherit") === "inherit"
            ? null
            : deno?.permissions,
        };

        // If the permission option is set to "none", all permissions
        // must be removed from the worker
        if (workerDenoAttributes.permissions === "none") {
          workerDenoAttributes.permissions = {
            env: false,
            hrtime: false,
            net: false,
            plugin: false,
            read: false,
            run: false,
            write: false,
          };
        }
      }

      if (type !== "module") {
        throw new Error(
          'Not yet implemented: only "module" type workers are supported',
        );
      }

      this.#name = name;
      const hasSourceCode = false;
      const sourceCode = core.decode(new Uint8Array());

      if (
        StringPrototypeStartsWith(specifier, "./") ||
        StringPrototypeStartsWith(specifier, "../") ||
        StringPrototypeStartsWith(specifier, "/") || type == "classic"
      ) {
        const baseUrl = getLocationHref();
        if (baseUrl != null) {
          specifier = new URL(specifier, baseUrl).href;
        }
      }

      const id = createWorker(
        specifier,
        hasSourceCode,
        sourceCode,
        workerDenoAttributes.namespace,
        workerDenoAttributes.permissions === null
          ? null
          : parsePermissions(workerDenoAttributes.permissions),
        options?.name,
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
      while (!this.#terminated) {
        const [type, data] = await hostRecvCtrl(this.#id);

        // If terminate was called then we ignore all messages
        if (this.#terminated) {
          return;
        }

        switch (type) {
          case 1: { // TerminalError
            this.#terminated = true;
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
            this.#terminated = true;
            return;
          }
          default: {
            throw new Error(`Unknown worker event: "${type}"`);
          }
        }
      }
    };

    #pollMessages = async () => {
      while (!this.terminated) {
        const data = await hostRecvMessage(this.#id);
        if (data === null) break;
        let message, transfer;
        try {
          const v = deserializeJsMessageData(data);
          message = v[0];
          transfer = v[1];
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
          ports: transfer,
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
        options = webidl.converters.PostMessageOptions(transferOrOptions, {
          prefix,
          context: "Argument 2",
        });
      }
      const { transfer } = options;
      const data = serializeJsMessageData(message, transfer);
      if (this.#terminated) return;
      hostPostMessage(this.#id, data);
    }

    terminate() {
      if (!this.#terminated) {
        this.#terminated = true;
        hostTerminateWorker(this.#id);
      }
    }
  }

  defineEventHandler(Worker.prototype, "error");
  defineEventHandler(Worker.prototype, "message");
  defineEventHandler(Worker.prototype, "messageerror");

  window.__bootstrap.worker = {
    parsePermissions,
    Worker,
  };
})(this);
