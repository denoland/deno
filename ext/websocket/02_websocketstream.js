// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file camelcase
/// <reference path="../../core/internal.d.ts" />

const core = globalThis.Deno.core;
const ops = core.ops;
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { Deferred, writableStreamClose } from "ext:deno_web/06_streams.js";
import DOMException from "ext:deno_web/01_dom_exception.js";
import { add, remove } from "ext:deno_web/03_abort_signal.js";
import {
  fillHeaders,
  headerListFromHeaders,
  headersFromHeaderList,
} from "ext:deno_fetch/20_headers.js";
import {
  _idleTimeoutDuration,
  _idleTimeoutTimeout,
  _rid,
  _server,
  _serverHandleIdleTimeout,
} from "ext:deno_websocket/01_websocket.js";
const primordials = globalThis.__bootstrap.primordials;
const {
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  DateNow,
  Error,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeCatch,
  PromisePrototypeThen,
  SafeSet,
  SetPrototypeGetSize,
  StringPrototypeEndsWith,
  StringPrototypeToLowerCase,
  Symbol,
  SymbolFor,
  TypeError,
  TypedArrayPrototypeGetByteLength,
  Uint8ArrayPrototype,
} = primordials;
const {
  op_ws_send_text_async,
  op_ws_send_binary_async,
  op_ws_send_ping,
  op_ws_next_event,
  op_ws_get_buffer,
  op_ws_get_buffer_as_string,
  op_ws_get_error,
  op_ws_create,
  op_ws_close,
} = core.ensureFastOps();

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
    {
      key: "headers",
      converter: webidl.converters.HeadersInit,
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

const CLOSE_RESPONSE_TIMEOUT = 5000;

const _url = Symbol("[[url]]");
const _connection = Symbol("[[connection]]");
const _closed = Symbol("[[closed]]");
const _earlyClose = Symbol("[[earlyClose]]");
const _closeSent = Symbol("[[closeSent]]");
const _initStreams = Symbol("[[initStreams]]");

class WebSocketStream {
  [_rid];

  [_url];
  get url() {
    webidl.assertBranded(this, WebSocketStreamPrototype);
    return this[_url];
  }

  constructor(url, options) {
    this[webidl.brand] = webidl.brand;
    this[_idleTimeoutDuration] = 0;
    this[_idleTimeoutTimeout] = undefined;
    const prefix = "Failed to construct 'WebSocketStream'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    url = webidl.converters.USVString(url, prefix, "Argument 1");
    options = webidl.converters.WebSocketStreamOptions(
      options,
      prefix,
      "Argument 2",
    );

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
        SetPrototypeGetSize(
          new SafeSet(
            ArrayPrototypeMap(
              options.protocols,
              (p) => StringPrototypeToLowerCase(p),
            ),
          ),
        )
    ) {
      throw new DOMException(
        "Can't supply multiple times the same protocol.",
        "SyntaxError",
      );
    }

    const headers = headersFromHeaderList([], "request");
    if (options.headers !== undefined) {
      fillHeaders(headers, options.headers);
    }

    const cancelRid = ops.op_ws_check_permission_and_cancel_handle(
      "WebSocketStream.abort()",
      this[_url],
      true,
    );

    if (options.signal?.aborted) {
      core.close(cancelRid);
      const err = options.signal.reason;
      this[_connection].reject(err);
      this[_closed].reject(err);
    } else {
      const abort = () => {
        core.close(cancelRid);
      };
      options.signal?.[add](abort);
      PromisePrototypeThen(
        op_ws_create(
          "new WebSocketStream()",
          this[_url],
          options.protocols ? ArrayPrototypeJoin(options.protocols, ", ") : "",
          cancelRid,
          headerListFromHeaders(headers),
        ),
        (create) => {
          options.signal?.[remove](abort);
          if (this[_earlyClose]) {
            PromisePrototypeThen(
              op_ws_close(create.rid),
              () => {
                PromisePrototypeThen(
                  (async () => {
                    while (true) {
                      const kind = await op_ws_next_event(create.rid);

                      if (kind > 5) {
                        /* close */
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

            const { readable, writable } = this[_initStreams]();

            this[_connection].resolve({
              readable,
              writable,
              extensions: create.extensions ?? "",
              protocol: create.protocol ?? "",
            });
          }
        },
        (err) => {
          if (ObjectPrototypeIsPrototypeOf(core.InterruptedPrototype, err)) {
            // The signal was aborted.
            err = options.signal.reason;
          } else {
            core.tryClose(cancelRid);
          }
          this[_connection].reject(err);
          this[_closed].reject(err);
        },
      );
    }
  }

  [_initStreams]() {
    const writable = new WritableStream({
      write: async (chunk) => {
        if (typeof chunk === "string") {
          await op_ws_send_text_async(this[_rid], chunk);
        } else if (
          ObjectPrototypeIsPrototypeOf(Uint8ArrayPrototype, chunk)
        ) {
          await op_ws_send_binary_async(this[_rid], chunk);
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
    const pull = async (controller) => {
      // Remember that this pull method may be re-entered before it has completed
      const kind = await op_ws_next_event(this[_rid]);
      switch (kind) {
        case 0:
          /* string */
          this[_serverHandleIdleTimeout]();
          controller.enqueue(op_ws_get_buffer_as_string(this[_rid]));
          break;
        case 1: {
          /* binary */
          this[_serverHandleIdleTimeout]();
          controller.enqueue(op_ws_get_buffer(this[_rid]));
          break;
        }
        case 2: {
          /* pong */
          this[_serverHandleIdleTimeout]();
          return pull(controller);
          // break;
        }
        case 3: {
          /* error */
          const err = new Error(op_ws_get_error(this[_rid]));
          this[_closed].reject(err);
          controller.error(err);
          core.tryClose(this[_rid]);
          break;
        }
        case 1005: {
          /* closed */
          this[_closed].resolve({ code: 1005, reason: "" });
          clearTimeout(this[_idleTimeoutTimeout]);
          core.tryClose(this[_rid]);
          break;
        }
        default: {
          /* close */
          const reason = op_ws_get_error(this[_rid]);
          this[_closed].resolve({
            code: kind,
            reason,
          });
          clearTimeout(this[_idleTimeoutTimeout]);
          core.tryClose(this[_rid]);
          break;
        }
      }

      if (
        this[_closeSent].state === "fulfilled" &&
        this[_closed].state === "pending"
      ) {
        if (
          DateNow() - await this[_closeSent].promise <=
            CLOSE_RESPONSE_TIMEOUT
        ) {
          return pull(controller);
        }

        const error = op_ws_get_error(this[_rid]);
        this[_closed].reject(new Error(error));
        core.tryClose(this[_rid]);
      }
    };
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

        PromisePrototypeThen(this[_closeSent].promise, () => {
          if (this[_closed].state === "pending") {
            return pull(controller);
          }
        });
      },
      pull,
      cancel: async (reason) => {
        try {
          this.close(reason?.code !== undefined ? reason : {});
        } catch (_) {
          this.close();
        }
        await this.closed;
      },
    });

    return { readable, writable };
  }

  [_connection] = new Deferred();
  get connection() {
    webidl.assertBranded(this, WebSocketStreamPrototype);
    return this[_connection].promise;
  }

  [_earlyClose] = false;
  [_closed] = new Deferred();
  [_closeSent] = new Deferred();
  get closed() {
    webidl.assertBranded(this, WebSocketStreamPrototype);
    return this[_closed].promise;
  }

  close(closeInfo) {
    webidl.assertBranded(this, WebSocketStreamPrototype);
    closeInfo = webidl.converters.WebSocketCloseInfo(
      closeInfo,
      "Failed to execute 'close' on 'WebSocketStream'",
      "Argument 1",
    );

    if (!this[_server]) {
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
    }

    const encoder = new TextEncoder();
    if (
      closeInfo.reason &&
      TypedArrayPrototypeGetByteLength(encoder.encode(closeInfo.reason)) > 123
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
      PromisePrototypeThen(
        op_ws_close(this[_rid], code, closeInfo.reason),
        () => {
          setTimeout(() => {
            this[_closeSent].resolve(DateNow());
          }, 0);
        },
        (err) => {
          this[_rid] && core.tryClose(this[_rid]);
          this[_closed].reject(err);
        },
      );
    }
  }

  [_serverHandleIdleTimeout]() {
    if (this[_idleTimeoutDuration]) {
      clearTimeout(this[_idleTimeoutTimeout]);
      this[_idleTimeoutTimeout] = setTimeout(async () => {
        if (this[_connection].state === "fulfilled" && this[_closed].state === "pending") {
          await op_ws_send_ping(this[_rid]);
          this[_idleTimeoutTimeout] = setTimeout(async () => {
            if (this[_connection].state === "fulfilled" && this[_closed].state === "pending") {
              const reason = "No response from ping frame.";
              await op_ws_close(this[_rid], 1001, reason);
              this[_closed].reject(new Error(reason));
              core.tryClose(this[_rid]);
            } else {
              clearTimeout(this[_idleTimeoutTimeout]);
            }
          }, (this[_idleTimeoutDuration] / 2) * 1000);
        } else {
          clearTimeout(this[_idleTimeoutTimeout]);
        }
      }, (this[_idleTimeoutDuration] / 2) * 1000);
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

const WebSocketStreamPrototype = WebSocketStream.prototype;

export {
  _closed,
  _closeSent,
  _connection,
  _initStreams,
  WebSocketStream,
};
