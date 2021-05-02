// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

  webidl.converters["sequence<USVString>"] = webidl.createSequenceConverter(
    webidl.converters["USVString"],
  );
  webidl.converters.AbortSignal = webidl.createInterfaceConverter(
    "AbortSignal",
    AbortSignal,
  );
  webidl.converters.WebSocketStreamOptions = webidl.createDictionaryConverter(
    "WebSocketStreamOptions",
    [
      {
        key: "protocols",
        converter: webidl.converters["sequence<USVString>"],
        defaultValue: [],
      },
      {
        key: "signal",
        converter: webidl.converters.AbortSignal,
        defaultValue: undefined,
      },
    ],
  );
  webidl.converters.WebSocketCloseInfo = webidl.createDictionaryConverter(
    "WebSocketStreamOptions",
    [
      {
        key: "code",
        converter: webidl.converters["unsigned short"],
        defaultValue: undefined,
      },
      {
        key: "reason",
        converter: webidl.converters.USVString,
        defaultValue: "",
      },
    ],
  );

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

  const _rid = Symbol("[[rid]]");
  const _url = Symbol("[[url]]");
  const _connection = Symbol("[[connection]]");
  const _closed = Symbol("[[_closed]]");
  class WebSocketStream {
    [_rid];

    [_url];
    get url() {
      return this[_url];
    }

    constructor(url, options) {
      const prefix = "Failed to construct 'WebSocketStream'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      url = webidl.converters.USVString(url, {
        prefix,
        context: "Argument 1",
      });
      options = webidl.converters.WebSocketStreamOptions(options, {
        prefix,
        context: "Argument 1",
      });

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

      this[_url] = wsURL.href;

      core.opSync("op_ws_check_permission", this[_url]);

      if (
        options.protocols.some((x) =>
          options.protocols.indexOf(x) !== options.protocols.lastIndexOf(x)
        )
      ) {
        throw new DOMException(
          "Can't supply multiple times the same protocol.",
          "SyntaxError",
        );
      }

      core.opAsync("op_ws_create", {
        url: this[_url],
        protocols: options.protocols.join(", "),
      }).then((create) => {
        options.abort?.addEventListener("abort", () => this.close());

        this[_rid] = create.rid;
        const readable = new ReadableStream({
          pull: async (controller) => {
            const { kind, value } = await core.opAsync(
              "op_ws_next_event",
              this[_rid],
            );

            switch (kind) {
              case "string": {
                controller.enqueue(value);
                break;
              }
              case "binary": {
                controller.enqueue(value);
                break;
              }
              case "ping": {
                core.opAsync("op_ws_send", {
                  rid: this[_rid],
                  kind: "pong",
                });
                break;
              }
              case "close": {
                this[_closed].resolve(value);
                tryClose(this[_rid]);
                break;
              }
              case "error": {
                const err = new Error(value);
                this[_closed].reject(err);
                controller.error(err);
                tryClose(this[_rid]);
                break;
              }
            }
          },
          cancel: (reason) => this.close(reason),
        });
        const writable = new WritableStream({
          write: async (chunk) => {
            if (typeof chunk === "string") {
              await core.opAsync("op_ws_send", {
                rid: this[_rid],
                kind: "text",
                text: chunk,
              });
            } else if (chunk instanceof Uint8Array) {
              await core.opAsync("op_ws_send", {
                rid: this[_rid],
                kind: "binary",
              }, chunk);
            }
          },
          cancel: (reason) => this.close(reason),
          abort: (reason) => this.close(reason),
        });

        this[_connection].resolve({
          readable,
          writable,
          extensions: create.extensions ?? "",
          protocol: create.protocol ?? "",
        });
      }).catch((err) => {
        this[_connection].reject(err);
        this[_closed].reject(err);
      });
    }

    [_connection] = new Deferred();
    get connection() {
      return this[_connection].promise;
    }

    [_closed] = new Deferred();
    get closed() {
      return this[_closed].promise;
    }

    close(closeInfo) {
      closeInfo = webidl.converters.WebSocketCloseInfo(closeInfo, {
        prefix: "Failed to execute 'close' on 'WebSocketStream'",
        context: "Argument 1",
      });

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

      if (this[_closed].state === "pending") {
        core.opAsync("op_ws_close", {
          rid: this[_rid],
          code,
          reason: closeInfo?.reason,
        }).then(() => {
          tryClose(this[_rid]);
          this[_closed].resolve({
            code: closeInfo?.code,
            reason: closeInfo?.reason,
          });
        }).catch((err) => {
          tryClose(this[_rid]);
          this[_closed].reject(err);
        });
      }
    }
  }

  window.__bootstrap.webSocket.WebSocketStream = WebSocketStream;
})(this);
