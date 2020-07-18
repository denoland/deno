// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */

((window) => {
  const { log } = window.__bootstrap.util;
  const { sendSync, sendAsync } = window.__bootstrap.dispatchJson;
  /*
  import { blobURLMap } from "./web/url.ts";
  */

  function createWorker(
    specifier,
    hasSourceCode,
    sourceCode,
    useDenoNamespace,
    name,
  ) {
    return sendSync("op_create_worker", {
      specifier,
      hasSourceCode,
      sourceCode,
      name,
      useDenoNamespace,
    });
  }

  function hostTerminateWorker(id) {
    sendSync("op_host_terminate_worker", { id });
  }

  function hostPostMessage(id, data) {
    sendSync("op_host_post_message", { id }, data);
  }

  function hostGetMessage(id) {
    return sendAsync("op_host_get_message", { id });
  }

  const encoder = new TextEncoder();
  const decoder = new TextDecoder();

  class MessageEvent extends Event {
    constructor(type, eventInitDict) {
      super(type, {
        bubbles: eventInitDict?.bubbles ?? false,
        cancelable: eventInitDict?.cancelable ?? false,
        composed: eventInitDict?.composed ?? false,
      });

      this.data = eventInitDict?.data ?? null;
      this.origin = eventInitDict?.origin ?? "";
      this.lastEventId = eventInitDict?.lastEventId ?? "";
    }
  }

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

      /* TODO(bartlomieju):
      // Handle blob URL.
      if (specifier.startsWith("blob:")) {
        hasSourceCode = true;
        const b = blobURLMap.get(specifier);
        if (!b) {
          throw new Error("No Blob associated with the given URL is found");
        }
        const blobBytes = blobBytesWeakMap.get(b!);
        if (!blobBytes) {
          throw new Error("Invalid Blob");
        }
        sourceCode = blobBytes!;
      }
      */

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
        if (this.onmessageerror) {
          this.onmessageerror(msgErrorEvent);
        }
        return;
      }

      const msgEvent = new MessageEvent("message", {
        cancelable: false,
        data,
      });

      if (this.onmessage) {
        this.onmessage(msgEvent);
      }

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
      if (this.onerror) {
        this.onerror(event);
      }

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
            throw Error(event.error.message);
          }
          continue;
        }

        if (type === "msg") {
          this.#handleMessage(event.data);
          continue;
        }

        if (type === "error") {
          if (!this.#handleError(event.error)) {
            throw Error(event.error.message);
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

  window.__bootstrap.worker = {
    Worker,
    MessageEvent,
  };
})(this);
