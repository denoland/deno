// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

import { core, primordials } from "ext:core/mod.js";
import {
  op_ws_check_permission_and_cancel_handle,
  op_ws_close,
  op_ws_create,
  op_ws_get_buffer,
  op_ws_get_buffer_as_string,
  op_ws_get_error,
  op_ws_next_event,
  op_ws_send_binary_async,
  op_ws_send_text_async,
} from "ext:core/ops";
const {
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  DateNow,
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
  TypedArrayPrototypeGetSymbolToStringTag,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import { Deferred, writableStreamClose } from "ext:deno_web/06_streams.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { add, remove } from "ext:deno_web/03_abort_signal.js";
import {
  fillHeaders,
  headerListFromHeaders,
  headersFromHeaderList,
} from "ext:deno_fetch/20_headers.js";

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
      key: "closeCode",
      converter: (V, prefix, context, opts) =>
        webidl.converters["unsigned short"](V, prefix, context, {
          ...opts,
          enforceRange: true,
        }),
    },
    {
      key: "reason",
      converter: webidl.converters.USVString,
      defaultValue: "",
    },
  ],
);

const CLOSE_RESPONSE_TIMEOUT = 5000;

const _rid = Symbol("[[rid]]");
const _url = Symbol("[[url]]");
const _opened = Symbol("[[opened]]");
const _closed = Symbol("[[closed]]");
const _earlyClose = Symbol("[[earlyClose]]");
const _closeSent = Symbol("[[closeSent]]");
class WebSocketStream {
  [_rid];

  [_url];
  get url() {
    webidl.assertBranded(this, WebSocketStreamPrototype);
    return this[_url];
  }

  constructor(url, options) {
    this[webidl.brand] = webidl.brand;
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

    const cancelRid = op_ws_check_permission_and_cancel_handle(
      "WebSocketStream.abort()",
      this[_url],
      true,
    );

    if (options.signal?.aborted) {
      core.close(cancelRid);
      const err = options.signal.reason;
      this[_opened].reject(err);
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
                    const err = new WebSocketError("Closed while connecting");
                    this[_opened].reject(err);
                    this[_closed].reject(err);
                  },
                );
              },
              () => {
                const err = new WebSocketError("Closed while connecting");
                this[_opened].reject(err);
                this[_closed].reject(err);
              },
            );
          } else {
            this[_rid] = create.rid;

            const writable = new WritableStream({
              write: async (chunk) => {
                if (typeof chunk === "string") {
                  await op_ws_send_text_async(this[_rid], chunk);
                } else if (
                  TypedArrayPrototypeGetSymbolToStringTag(chunk) ===
                    "Uint8Array"
                ) {
                  await op_ws_send_binary_async(this[_rid], chunk);
                } else {
                  throw new TypeError(
                    "A chunk may only be either a string or an Uint8Array",
                  );
                }
              },
              close: async () => {
                this.close();
                await this.closed;
              },
              abort: async (reason) => {
                let closeCode = null;
                let reasonString = "";

                if (
                  ObjectPrototypeIsPrototypeOf(WebSocketErrorPrototype, reason)
                ) {
                  closeCode = reason.closeCode;
                  reasonString = reason.reason;
                }

                try {
                  this.close({
                    closeCode,
                    reason: reasonString,
                  });
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
                  controller.enqueue(op_ws_get_buffer_as_string(this[_rid]));
                  break;
                case 1: {
                  /* binary */
                  controller.enqueue(op_ws_get_buffer(this[_rid]));
                  break;
                }
                case 2: {
                  /* pong */
                  break;
                }
                case 3: {
                  /* error */
                  const err = new WebSocketError(op_ws_get_error(this[_rid]));
                  this[_closed].reject(err);
                  controller.error(err);
                  core.tryClose(this[_rid]);
                  break;
                }
                case 1005: {
                  /* closed */
                  this[_closed].resolve({ closeCode: 1005, reason: "" });
                  core.tryClose(this[_rid]);
                  break;
                }
                default: {
                  /* close */
                  const reason = op_ws_get_error(this[_rid]);
                  this[_closed].resolve({
                    closeCode: kind,
                    reason,
                  });
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
                this[_closed].reject(new WebSocketError(error));
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
                let closeCode = null;
                let reasonString = "";

                if (
                  ObjectPrototypeIsPrototypeOf(WebSocketErrorPrototype, reason)
                ) {
                  closeCode = reason.closeCode;
                  reasonString = reason.reason;
                }

                try {
                  this.close({
                    closeCode,
                    reason: reasonString,
                  });
                } catch (_) {
                  this.close();
                }
                await this.closed;
              },
            });

            this[_opened].resolve({
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
            err = new WebSocketError(err.message);
          }
          this[_opened].reject(err);
          this[_closed].reject(err);
        },
      );
    }
  }

  [_opened] = new Deferred();
  get opened() {
    webidl.assertBranded(this, WebSocketStreamPrototype);
    return this[_opened].promise;
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

    validateCloseCodeAndReason(closeInfo);

    if (closeInfo.reason && closeInfo.closeCode === null) {
      closeInfo.closeCode = 1000;
    }

    if (this[_opened].state === "pending") {
      this[_earlyClose] = true;
    } else if (this[_closed].state === "pending") {
      PromisePrototypeThen(
        op_ws_close(this[_rid], closeInfo.closeCode, closeInfo.reason),
        () => {
          setTimeout(() => {
            this[_closeSent].resolve(DateNow());
          }, 0);
        },
        (err) => {
          this[_rid] && core.tryClose(this[_rid]);
          err = new WebSocketError(err.message);
          this[_closed].reject(err);
        },
      );
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(WebSocketStreamPrototype, this),
        keys: [
          "closed",
          "opened",
          "url",
        ],
      }),
      inspectOptions,
    );
  }
}
const WebSocketStreamPrototype = WebSocketStream.prototype;

function validateCloseCodeAndReason(closeInfo) {
  if (!closeInfo.closeCode) {
    closeInfo.closeCode = null;
  }

  if (
    closeInfo.closeCode &&
    !(closeInfo.closeCode === 1000 ||
      (3000 <= closeInfo.closeCode && closeInfo.closeCode < 5000))
  ) {
    throw new DOMException(
      "The close code must be either 1000 or in the range of 3000 to 4999.",
      "InvalidAccessError",
    );
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
}

class WebSocketError extends DOMException {
  #closeCode;
  #reason;

  constructor(message = "", init = { __proto__: null }) {
    super(message, "WebSocketError");
    this[webidl.brand] = webidl.brand;

    init = webidl.converters["WebSocketCloseInfo"](
      init,
      "Failed to construct 'WebSocketError'",
      "Argument 2",
    );

    validateCloseCodeAndReason(init);

    if (init.reason && init.closeCode === null) {
      init.closeCode = 1000;
    }

    this.#closeCode = init.closeCode;
    this.#reason = init.reason;
  }

  get closeCode() {
    webidl.assertBranded(this, WebSocketErrorPrototype);
    return this.#closeCode;
  }

  get reason() {
    webidl.assertBranded(this, WebSocketErrorPrototype);
    return this.#reason;
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(WebSocketErrorPrototype, this),
        keys: [
          "message",
          "name",
          "closeCode",
          "reason",
        ],
      }),
      inspectOptions,
    );
  }
}
webidl.configureInterface(WebSocketError);
const WebSocketErrorPrototype = WebSocketError.prototype;

export { WebSocketError, WebSocketStream };
