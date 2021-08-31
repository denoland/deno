// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

/// <reference path="../../core/internal.d.ts" />

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;
  const { writableStreamClose, Deferred } = window.__bootstrap.streams;
  const { DOMException } = window.__bootstrap.domException;
  const { add, remove } = window.__bootstrap.abortSignal;

  const {
    StringPrototypeEndsWith,
    StringPrototypeToLowerCase,
    Symbol,
    SymbolFor,
    Set,
    ArrayPrototypeMap,
    ArrayPrototypeJoin,
    PromisePrototypeThen,
    PromisePrototypeCatch,
    Uint8Array,
    TypeError,
    Error,
  } = window.__bootstrap.primordials;

  webidl.converters.WebSocketStreamOptions = webidl.createDictionaryConverter(
    "WebSocketStreamOptions",
    [
      {
        key: "protocols",
        converter: webidl.converters["sequence<USVString>"],
        get defaultValue() {
          return [];
        },
      },
      {
        key: "signal",
        converter: webidl.converters.AbortSignal,
      },
    ],
  );
  webidl.converters.WebSocketCloseInfo = webidl.createDictionaryConverter(
    "WebSocketCloseInfo",
    [
      {
        key: "code",
        converter: webidl.converters["unsigned short"],
      },
      {
        key: "reason",
        converter: webidl.converters.USVString,
        defaultValue: "",
      },
    ],
  );

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
  const _closed = Symbol("[[closed]]");
  const _closing = Symbol("[[closing]]");
  const _earlyClose = Symbol("[[earlyClose]]");
  class WebSocketStream {
    [_rid];

    [_url];
    get url() {
      webidl.assertBranded(this, WebSocketStream);
      return this[_url];
    }

    constructor(url, options) {
      this[webidl.brand] = webidl.brand;
      const prefix = "Failed to construct 'WebSocketStream'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      url = webidl.converters.USVString(url, {
        prefix,
        context: "Argument 1",
      });
      options = webidl.converters.WebSocketStreamOptions(options, {
        prefix,
        context: "Argument 2",
      });

      const wsURL = new URL(url);

      if (wsURL.protocol !== "ws:" && wsURL.protocol !== "wss:") {
        throw new DOMException(
          "Only ws & wss schemes are allowed in a WebSocket URL.",
          "SyntaxError",
        );
      }

      if (wsURL.hash !== "" || StringPrototypeEndsWith(wsURL.href, "#")) {
        throw new DOMException(
          "Fragments are not allowed in a WebSocket URL.",
          "SyntaxError",
        );
      }

      this[_url] = wsURL.href;

      if (
        options.protocols.length !==
          new Set(
            ArrayPrototypeMap(
              options.protocols,
              (p) => StringPrototypeToLowerCase(p),
            ),
          ).size
      ) {
        throw new DOMException(
          "Can't supply multiple times the same protocol.",
          "SyntaxError",
        );
      }

      const cancelRid = core.opSync(
        "op_ws_check_permission_and_cancel_handle",
        this[_url],
        true,
      );

      if (options.signal?.aborted) {
        core.close(cancelRid);
        const err = new DOMException(
          "This operation was aborted",
          "AbortError",
        );
        this[_connection].reject(err);
        this[_closed].reject(err);
      } else {
        const abort = () => {
          core.close(cancelRid);
        };
        options.signal?.[add](abort);
        PromisePrototypeThen(
          core.opAsync("op_ws_create", {
            url: this[_url],
            protocols: options.protocols
              ? ArrayPrototypeJoin(options.protocols, ", ")
              : "",
            cancelHandle: cancelRid,
          }),
          (create) => {
            options.signal?.[remove](abort);
            if (this[_earlyClose]) {
              PromisePrototypeThen(
                core.opAsync("op_ws_close", {
                  rid: create.rid,
                }),
                () => {
                  PromisePrototypeThen(
                    (async () => {
                      while (true) {
                        const { kind } = await core.opAsync(
                          "op_ws_next_event",
                          create.rid,
                        );

                        if (kind === "close") {
                          break;
                        }
                      }
                    })(),
                    () => {
                      const err = new DOMException(
                        "Closed while connecting",
                        "NetworkError",
                      );
                      this[_connection].reject(err);
                      this[_closed].reject(err);
                    },
                  );
                },
                () => {
                  const err = new DOMException(
                    "Closed while connecting",
                    "NetworkError",
                  );
                  this[_connection].reject(err);
                  this[_closed].reject(err);
                },
              );
            } else {
              this[_rid] = create.rid;

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
                  } else {
                    throw new TypeError(
                      "A chunk may only be either a string or an Uint8Array",
                    );
                  }
                },
                close: async (reason) => {
                  try {
                    this.close(reason?.code !== undefined ? reason : {});
                  } catch (_) {
                    this.close();
                  }
                  await this.closed;
                },
                abort: async (reason) => {
                  try {
                    this.close(reason?.code !== undefined ? reason : {});
                  } catch (_) {
                    this.close();
                  }
                  await this.closed;
                },
              });
              const readable = new ReadableStream({
                start: (controller) => {
                  PromisePrototypeThen(this.closed, () => {
                    try {
                      controller.close();
                    } catch (_) {
                      // needed to ignore warnings & assertions
                    }
                    try {
                      PromisePrototypeCatch(
                        writableStreamClose(writable),
                        () => {},
                      );
                    } catch (_) {
                      // needed to ignore warnings & assertions
                    }
                  });
                },
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
                      await core.opAsync("op_ws_send", {
                        rid: this[_rid],
                        kind: "pong",
                      });
                      break;
                    }
                    case "close": {
                      if (this[_closing]) {
                        this[_closed].resolve(value);
                        tryClose(this[_rid]);
                      } else {
                        PromisePrototypeThen(
                          core.opAsync("op_ws_close", {
                            rid: this[_rid],
                            ...value,
                          }),
                          () => {
                            this[_closed].resolve(value);
                            tryClose(this[_rid]);
                          },
                          (err) => {
                            this[_closed].reject(err);
                            controller.error(err);
                            tryClose(this[_rid]);
                          },
                        );
                      }
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
                cancel: async (reason) => {
                  try {
                    this.close(reason?.code !== undefined ? reason : {});
                  } catch (_) {
                    this.close();
                  }
                  await this.closed;
                },
              });

              this[_connection].resolve({
                readable,
                writable,
                extensions: create.extensions ?? "",
                protocol: create.protocol ?? "",
              });
            }
          },
          (err) => {
            tryClose(cancelRid);
            this[_connection].reject(err);
            this[_closed].reject(err);
          },
        );
      }
    }

    [_connection] = new Deferred();
    get connection() {
      webidl.assertBranded(this, WebSocketStream);
      return this[_connection].promise;
    }

    [_earlyClose] = false;
    [_closing] = false;
    [_closed] = new Deferred();
    get closed() {
      webidl.assertBranded(this, WebSocketStream);
      return this[_closed].promise;
    }

    close(closeInfo) {
      webidl.assertBranded(this, WebSocketStream);
      closeInfo = webidl.converters.WebSocketCloseInfo(closeInfo, {
        prefix: "Failed to execute 'close' on 'WebSocketStream'",
        context: "Argument 1",
      });

      if (
        closeInfo.code &&
        !(closeInfo.code === 1000 ||
          (3000 <= closeInfo.code && closeInfo.code < 5000))
      ) {
        throw new DOMException(
          "The close code must be either 1000 or in the range of 3000 to 4999.",
          "InvalidAccessError",
        );
      }

      const encoder = new TextEncoder();
      if (
        closeInfo.reason && encoder.encode(closeInfo.reason).byteLength > 123
      ) {
        throw new DOMException(
          "The close reason may not be longer than 123 bytes.",
          "SyntaxError",
        );
      }

      let code = closeInfo.code;
      if (closeInfo.reason && code === undefined) {
        code = 1000;
      }

      if (this[_connection].state === "pending") {
        this[_earlyClose] = true;
      } else if (this[_closed].state === "pending") {
        this[_closing] = true;
        PromisePrototypeCatch(
          core.opAsync("op_ws_close", {
            rid: this[_rid],
            code,
            reason: closeInfo.reason,
          }),
          (err) => {
            this[_rid] && tryClose(this[_rid]);
            this[_closed].reject(err);
          },
        );
      }
    }

    [SymbolFor("Deno.customInspect")](inspect) {
      return `${this.constructor.name} ${
        inspect({
          url: this.url,
        })
      }`;
    }
  }

  window.__bootstrap.webSocket.WebSocketStream = WebSocketStream;
})(this);
