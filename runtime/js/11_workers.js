// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const { Window } = window.__bootstrap.globalInterfaces;
  const { log } = window.__bootstrap.util;
  const { defineEventHandler } = window.__bootstrap.webUtil;

  function createWorker(
    specifier,
    hasSourceCode,
    sourceCode,
    useDenoNamespace,
    name,
  ) {
    return core.jsonOpSync("op_create_worker", {
      specifier,
      hasSourceCode,
      sourceCode,
      name,
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

  class Worker extends EventTarget {
    #id = 0;
    #name = "";
    #terminated = false;

    constructor(specifier, options) {
      super();
      const { type = "classic", name = "unknown" } = options ?? {};

      if (type !== "module") {
        throw new Error(
          'Not yet implemented: only "module" type workers are supported',
        );
      }

      this.#name = name;
      const hasSourceCode = false;
      const sourceCode = decoder.decode(new Uint8Array());

      const useDenoNamespace = options ? !!options.deno : false;

      const { id } = createWorker(
        specifier,
        hasSourceCode,
        sourceCode,
        useDenoNamespace,
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
        throw new Error(`Received data could not be parsed: '${msgData}'.`);
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
