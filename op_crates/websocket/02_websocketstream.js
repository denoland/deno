// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function requiredArguments(
    name,
    length,
    required,
  ) {
    if (length < required) {
      const errMsg = `${name} requires at least ${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present`;
      throw new TypeError(errMsg);
    }
  }

  /** @template T */
  class Deferred {
    /** @type {Promise<T>} */
    #promise;
    /** @type {(reject?: any) => void} */
    #reject;
    /** @type {(value: T | PromiseLike<T>) => void} */
    #resolve;
    /** @type {"pending" | "fulfilled"} */
    #state = "pending";

    constructor() {
      this.#promise = new Promise((resolve, reject) => {
        this.#resolve = resolve;
        this.#reject = reject;
      });
    }

    /** @returns {Promise<T>} */
    get promise() {
      return this.#promise;
    }

    /** @returns {"pending" | "fulfilled"} */
    get state() {
      return this.#state;
    }

    /** @param {any=} reason */
    reject(reason) {
      // already settled promises are a no-op
      if (this.#state !== "pending") {
        return;
      }
      this.#state = "fulfilled";
      this.#reject(reason);
    }

    /** @param {T | PromiseLike<T>} value */
    resolve(value) {
      // already settled promises are a no-op
      if (this.#state !== "pending") {
        return;
      }
      this.#state = "fulfilled";
      this.#resolve(value);
    }
  }

  /**
   * Tries to close the resource (and ignores BadResource errors).
   * @param {number} rid
   */
  function tryClose(rid) {
    try {
      core.close(rid);
    } catch (err) {
      // Ignore error if the socket has already been closed.
      if (!(err instanceof Deno.errors.BadResource)) throw err;
    }
  }

  class WebSocketStream {
    #rid;

    #url;
    get url() {
      return this.#url;
    }

    constructor(url, options) {
      requiredArguments("WebSocket", arguments.length, 1);

      const wsURL = new URL(url);

      if (wsURL.protocol !== "ws:" && wsURL.protocol !== "wss:") {
        throw new DOMException(
          "Only ws & wss schemes are allowed in a WebSocket URL.",
          "SyntaxError",
        );
      }

      if (wsURL.hash !== "" || wsURL.href.endsWith("#")) {
        throw new DOMException(
          "Fragments are not allowed in a WebSocket URL.",
          "SyntaxError",
        );
      }

      this.#url = wsURL.href;

      core.opSync("op_ws_check_permission", this.#url);

      if (
        options?.protocols?.some((x) =>
          options.protocols.indexOf(x) !== options.protocols.lastIndexOf(x)
        )
      ) {
        throw new DOMException(
          "Can't supply multiple times the same protocol.",
          "SyntaxError",
        );
      }

      core.opAsync("op_ws_create", {
        url: wsURL.href,
        protocols: options?.protocols?.join(", "),
      }).then((create) => {
        if (create.success) {
          options.abort.addEventListener("abort", () => this.close());

          const readable = new ReadableStream({
            pull: async (controller) => {
              const { kind, value } = await core.opAsync(
                "op_ws_next_event",
                this.#rid,
              );

              switch (kind) {
                case "string": {
                  controller.enqueue(value);
                  break;
                }
                case "binary": {
                  controller.enqueue(new Uint8Array(value));
                  break;
                }
                case "ping": {
                  core.opAsync("op_ws_send", {
                    rid: this.#rid,
                    kind: "pong",
                  });
                  break;
                }
                case "close": {
                  this.#closed.resolve(value);
                  tryClose(this.#rid);
                  break;
                }
                case "error": {
                  let err = new Error(value);
                  this.#closed.reject(err);
                  controller.error(err);
                  tryClose(this.#rid);
                  break;
                }
              }
            },
            cancel: (reason) => {
              this.close(reason);
            },
          });
          const writable = new WritableStream({
            write: async (chunk) => {
              if (typeof chunk === "string") {
                await core.opAsync("op_ws_send", {
                  rid: this.#rid,
                  kind: "text",
                  text: chunk,
                });
              } else if (chunk instanceof Uint8Array) {
                await core.opAsync("op_ws_send", {
                  rid: this.#rid,
                  kind: "binary",
                }, chunk);
              }
            },
            cancel: (reason) => {
              this.close(reason);
            },
            abort: (reason) => {
              this.close(reason);
            },
          });

          this.#connection.resolve({
            readable,
            writable,
            extensions: create.extensions ?? "",
            protocol: create.protocol ?? "",
          });
        } else {
          const err = new Error(create.error);
          this.#connection.reject(err);
          this.#closed.reject(err);
        }
      }).catch((err) => {
        this.#connection.reject(err);
        this.#closed.reject(err);
      });
    }

    #connection = new Deferred();
    get connection() {
      return this.#connection.promise;
    }

    #closed = new Deferred();
    get closed() {
      return this.#closed.promise;
    }

    close(closeInfo) {
      if (
        closeInfo?.code &&
        !(closeInfo.code === 1000 ||
          (3000 <= closeInfo.code && closeInfo.code < 5000))
      ) {
        throw new DOMException(
          "The close code must be either 1000 or in the range of 3000 to 4999.",
          "NotSupportedError",
        );
      }

      const encoder = new TextEncoder();
      if (
        closeInfo?.reason && encoder.encode(closeInfo.reason).byteLength > 123
      ) {
        throw new DOMException(
          "The close reason may not be longer than 123 bytes.",
          "SyntaxError",
        );
      }

      let code = closeInfo?.code;
      if (closeInfo?.reason && code === undefined) {
        code = 1000;
      }

      if (this.#closed.state === "pending") {
        core.opAsync("op_ws_close", {
          rid: this.#rid,
          code,
          reason: closeInfo?.reason,
        }).then(() => {
          this.#closed.resolve({
            code: closeInfo?.code ?? 1005,
            reason: closeInfo?.reason,
          });
        }).catch((err) => {
          this.#closed.reject(err);
        });
      }
    }
  }

  window.__bootstrap.webSocket.WebSocketStream = WebSocketStream;
})(this);
