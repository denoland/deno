// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { Window } = window.__bootstrap.globalInterfaces;
  const { getLocationHref } = window.__bootstrap.location;
  const { log, pathFromURL } = window.__bootstrap.util;
  const { defineEventHandler } = window.__bootstrap.webUtil;

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

  function hostGetMessage(id) {
    return core.opAsync("op_host_get_message", id);
  }

  const decoder = new TextDecoder();

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
    } else if (!Array.isArray(value) && typeof value !== "boolean") {
      throw new Error(
        `Expected 'array' or 'boolean' for ${permission} permission, ${typeof value} received`,
      );
      //Casts URLs to absolute routes
    } else if (Array.isArray(value)) {
      value = value.map((route) => {
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
      const sourceCode = decoder.decode(new Uint8Array());

      if (
        specifier.startsWith("./") || specifier.startsWith("../") ||
        specifier.startsWith("/") || type == "classic"
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
      this.#poll();
    }

    #handleMessage = (data) => {
      const msgEvent = new MessageEvent("message", {
        cancelable: false,
        data,
      });

      this.dispatchEvent(msgEvent);
    };

    #handleError = (e) => {
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
    };

    #poll = async () => {
      while (!this.#terminated) {
        const [type, data] = await hostGetMessage(this.#id);

        // If terminate was called then we ignore all messages
        if (this.#terminated) {
          return;
        }

        switch (type) {
          case 0: { // Message
            const msg = core.deserialize(data);
            this.#handleMessage(msg);
            break;
          }
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

    postMessage(message, transferOrOptions) {
      if (transferOrOptions) {
        throw new Error(
          "Not yet implemented: `transfer` and `options` are not supported.",
        );
      }

      if (this.#terminated) {
        return;
      }

      const bufferMsg = core.serialize(message);
      hostPostMessage(this.#id, bufferMsg);
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
