// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const { Window } = window.__bootstrap.globalInterfaces;
  const { log, pathFromURL } = window.__bootstrap.util;
  const { defineEventHandler } = window.__bootstrap.webUtil;
  const build = window.__bootstrap.build.build;

  function createWorker(
    specifier,
    hasSourceCode,
    sourceCode,
    useDenoNamespace,
    permissions,
    name,
  ) {
    return core.jsonOpSync("op_create_worker", {
      hasSourceCode,
      name,
      permissions,
      sourceCode,
      specifier,
      useDenoNamespace,
    });
  }

  function hostTerminateWorker(id) {
    core.jsonOpSync("op_host_terminate_worker", { id });
  }

  function hostPostMessage(id, data) {
    core.jsonOpSync("op_host_post_message", { id }, data);
  }

  function hostGetMessage(id) {
    return core.jsonOpAsync("op_host_get_message", { id });
  }

  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  function encodeMessage(data) {
    const dataJson = JSON.stringify(data);
    return encoder.encode(dataJson);
  }

  function decodeMessage(dataIntArray) {
    const dataJson = decoder.decode(dataIntArray);
    return JSON.parse(dataJson);
  }

  /**
   * @param {string} permission
   * @return {boolean}
   */
  function parseBooleanPermission(
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
      env: parseBooleanPermission(env, "env"),
      hrtime: parseBooleanPermission(hrtime, "hrtime"),
      net: parseArrayPermission(net, "net"),
      plugin: parseBooleanPermission(plugin, "plugin"),
      read: parseArrayPermission(read, "read"),
      run: parseBooleanPermission(run, "run"),
      write: parseArrayPermission(write, "write"),
    };
  }

  class Worker extends EventTarget {
    #id = 0;
    #name = "";
    #terminated = false;

    constructor(specifier, options = {}) {
      super();
      const {
        deno = {},
        name = "unknown",
        type = "classic",
      } = options;

      // TODO(Soremwar)
      // `deno: true` is kept for backwards compatibility with the previous worker
      // options implementation. Remove for 2.0
      let workerDenoAttributes;
      if (deno === true) {
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

        // If the permission option is set to false, all permissions
        // must be removed from the worker
        if (workerDenoAttributes.permissions === false) {
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

      const { id } = createWorker(
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

    #handleMessage = (msgData) => {
      let data;
      try {
        data = decodeMessage(new Uint8Array(msgData));
      } catch (e) {
        const msgErrorEvent = new MessageEvent("messageerror", {
          cancelable: false,
          data,
        });
        return;
      }

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
        const event = await hostGetMessage(this.#id);

        // If terminate was called then we ignore all messages
        if (this.#terminated) {
          return;
        }

        const type = event.type;

        if (type === "terminalError") {
          this.#terminated = true;
          if (!this.#handleError(event.error)) {
            if (globalThis instanceof Window) {
              throw new Error("Unhandled error event reached main worker.");
            } else {
              core.jsonOpSync(
                "op_host_unhandled_error",
                { message: event.error.message },
              );
            }
          }
          continue;
        }

        if (type === "msg") {
          this.#handleMessage(event.data);
          continue;
        }

        if (type === "error") {
          if (!this.#handleError(event.error)) {
            if (globalThis instanceof Window) {
              throw new Error("Unhandled error event reached main worker.");
            } else {
              core.jsonOpSync(
                "op_host_unhandled_error",
                { message: event.error.message },
              );
            }
          }
          continue;
        }

        if (type === "close") {
          log(`Host got "close" message from worker: ${this.#name}`);
          this.#terminated = true;
          return;
        }

        throw new Error(`Unknown worker event: "${type}"`);
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

      hostPostMessage(this.#id, encodeMessage(message));
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
    Worker,
  };
})(this);
