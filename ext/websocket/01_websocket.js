// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference path="../../core/internal.d.ts" />

import { core, primordials } from "ext:core/mod.js";
const {
  isAnyArrayBuffer,
  isArrayBuffer,
} = core;
import {
  op_ws_check_permission_and_cancel_handle,
  op_ws_close,
  op_ws_create,
  op_ws_get_buffer,
  op_ws_get_buffer_as_string,
  op_ws_get_buffered_amount,
  op_ws_get_error,
  op_ws_next_event,
  op_ws_send_binary,
  op_ws_send_binary_ab,
  op_ws_send_ping,
  op_ws_send_text,
} from "ext:core/ops";
const {
  ArrayBufferIsView,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeShift,
  ArrayPrototypeSome,
  ErrorPrototypeToString,
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeCatch,
  PromisePrototypeThen,
  RegExpPrototypeExec,
  SafeSet,
  SetPrototypeGetSize,
  String,
  StringPrototypeEndsWith,
  StringPrototypeToLowerCase,
  Symbol,
  SymbolFor,
  SymbolIterator,
  TypedArrayPrototypeGetByteLength,
} = primordials;

import { URL } from "ext:deno_url/00_url.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";
import { HTTP_TOKEN_CODE_POINT_RE } from "ext:deno_web/00_infra.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";
import { clearTimeout, setTimeout } from "ext:deno_web/02_timers.js";
import {
  CloseEvent,
  defineEventHandler,
  dispatch,
  ErrorEvent,
  Event,
  EventTarget,
  MessageEvent,
  setIsTrusted,
} from "ext:deno_web/02_event.js";
import { Blob, BlobPrototype } from "ext:deno_web/09_file.js";
import { getLocationHref } from "ext:deno_web/12_location.js";

webidl.converters["sequence<DOMString> or DOMString"] = (
  V,
  prefix,
  context,
  opts,
) => {
  // Union for (sequence<DOMString> or DOMString)
  if (webidl.type(V) === "Object" && V !== null) {
    if (V[SymbolIterator] !== undefined) {
      return webidl.converters["sequence<DOMString>"](V, prefix, context, opts);
    }
  }
  return webidl.converters.DOMString(V, prefix, context, opts);
};

webidl.converters["WebSocketSend"] = (V, prefix, context, opts) => {
  // Union for (Blob or ArrayBufferView or ArrayBuffer or USVString)
  if (ObjectPrototypeIsPrototypeOf(BlobPrototype, V)) {
    return webidl.converters["Blob"](V, prefix, context, opts);
  }
  if (typeof V === "object") {
    if (isAnyArrayBuffer(V)) {
      return webidl.converters["ArrayBuffer"](V, prefix, context, opts);
    }
    if (ArrayBufferIsView(V)) {
      return webidl.converters["ArrayBufferView"](V, prefix, context, opts);
    }
  }
  return webidl.converters["USVString"](V, prefix, context, opts);
};

/** role */
const SERVER = 0;
const CLIENT = 1;

/** state */
const CONNECTING = 0;
const OPEN = 1;
const CLOSING = 2;
const CLOSED = 3;

const _readyState = Symbol("[[readyState]]");
const _url = Symbol("[[url]]");
const _rid = Symbol("[[rid]]");
const _role = Symbol("[[role]]");
const _extensions = Symbol("[[extensions]]");
const _protocol = Symbol("[[protocol]]");
const _binaryType = Symbol("[[binaryType]]");
const _eventLoop = Symbol("[[eventLoop]]");
const _sendQueue = Symbol("[[sendQueue]]");
const _queueSend = Symbol("[[queueSend]]");

const _server = Symbol("[[server]]");
const _idleTimeoutDuration = Symbol("[[idleTimeout]]");
const _idleTimeoutTimeout = Symbol("[[idleTimeoutTimeout]]");
const _serverHandleIdleTimeout = Symbol("[[serverHandleIdleTimeout]]");

class WebSocket extends EventTarget {
  constructor(url, protocols = []) {
    super();
    this[webidl.brand] = webidl.brand;
    this[_rid] = undefined;
    this[_role] = undefined;
    this[_readyState] = CONNECTING;
    this[_extensions] = "";
    this[_protocol] = "";
    this[_url] = "";
    this[_binaryType] = "blob";
    this[_idleTimeoutDuration] = 0;
    this[_idleTimeoutTimeout] = undefined;
    this[_sendQueue] = [];

    const prefix = "Failed to construct 'WebSocket'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    url = webidl.converters.USVString(url, prefix, "Argument 1");
    protocols = webidl.converters["sequence<DOMString> or DOMString"](
      protocols,
      prefix,
      "Argument 2",
    );

    let wsURL;

    try {
      wsURL = new URL(url, getLocationHref());
    } catch (e) {
      throw new DOMException(e.message, "SyntaxError");
    }

    if (wsURL.protocol === "http:") {
      wsURL.protocol = "ws:";
    } else if (wsURL.protocol === "https:") {
      wsURL.protocol = "wss:";
    }

    if (wsURL.protocol !== "ws:" && wsURL.protocol !== "wss:") {
      throw new DOMException(
        `Only ws & wss schemes are allowed in a WebSocket URL: received ${wsURL.protocol}`,
        "SyntaxError",
      );
    }

    if (wsURL.hash !== "" || StringPrototypeEndsWith(wsURL.href, "#")) {
      throw new DOMException(
        "Fragments are not allowed in a WebSocket URL",
        "SyntaxError",
      );
    }

    this[_url] = wsURL.href;
    this[_role] = CLIENT;

    op_ws_check_permission_and_cancel_handle(
      "WebSocket.abort()",
      this[_url],
      false,
    );

    if (typeof protocols === "string") {
      protocols = [protocols];
    }

    if (
      protocols.length !==
        SetPrototypeGetSize(
          new SafeSet(
            ArrayPrototypeMap(protocols, (p) => StringPrototypeToLowerCase(p)),
          ),
        )
    ) {
      throw new DOMException(
        "Cannot supply multiple times the same protocol",
        "SyntaxError",
      );
    }

    if (
      ArrayPrototypeSome(
        protocols,
        (protocol) =>
          RegExpPrototypeExec(HTTP_TOKEN_CODE_POINT_RE, protocol) === null,
      )
    ) {
      throw new DOMException(
        "Invalid protocol value",
        "SyntaxError",
      );
    }

    PromisePrototypeThen(
      op_ws_create(
        "new WebSocket()",
        wsURL.href,
        ArrayPrototypeJoin(protocols, ", "),
      ),
      (create) => {
        this[_rid] = create.rid;
        this[_extensions] = create.extensions;
        this[_protocol] = create.protocol;

        if (this[_readyState] === CLOSING) {
          PromisePrototypeThen(
            op_ws_close(this[_rid]),
            () => {
              this[_readyState] = CLOSED;

              const errEvent = new ErrorEvent("error");
              this.dispatchEvent(errEvent);

              const event = new CloseEvent("close");
              this.dispatchEvent(event);
              core.tryClose(this[_rid]);
            },
          );
        } else {
          this[_readyState] = OPEN;
          const event = new Event("open");
          this.dispatchEvent(event);

          this[_eventLoop]();
        }
      },
      (err) => {
        this[_readyState] = CLOSED;

        const errorEv = new ErrorEvent(
          "error",
          { error: err, message: ErrorPrototypeToString(err) },
        );
        this.dispatchEvent(errorEv);

        const closeEv = new CloseEvent("close");
        this.dispatchEvent(closeEv);
      },
    );
  }

  get readyState() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_readyState];
  }

  get CONNECTING() {
    webidl.assertBranded(this, WebSocketPrototype);
    return CONNECTING;
  }
  get OPEN() {
    webidl.assertBranded(this, WebSocketPrototype);
    return OPEN;
  }
  get CLOSING() {
    webidl.assertBranded(this, WebSocketPrototype);
    return CLOSING;
  }
  get CLOSED() {
    webidl.assertBranded(this, WebSocketPrototype);
    return CLOSED;
  }

  get extensions() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_extensions];
  }

  get protocol() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_protocol];
  }

  get url() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_url];
  }

  get binaryType() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_binaryType];
  }
  set binaryType(value) {
    webidl.assertBranded(this, WebSocketPrototype);
    value = webidl.converters.DOMString(
      value,
      "Failed to set 'binaryType' on 'WebSocket'",
    );
    if (value === "blob" || value === "arraybuffer") {
      this[_binaryType] = value;
    }
  }

  get bufferedAmount() {
    webidl.assertBranded(this, WebSocketPrototype);
    if (this[_readyState] === OPEN) {
      return op_ws_get_buffered_amount(this[_rid]);
    } else {
      return 0;
    }
  }

  send(data) {
    webidl.assertBranded(this, WebSocketPrototype);
    const prefix = "Failed to execute 'send' on 'WebSocket'";

    webidl.requiredArguments(arguments.length, 1, prefix);
    data = webidl.converters.WebSocketSend(data, prefix, "Argument 1");

    if (this[_readyState] !== OPEN) {
      throw new DOMException("'readyState' not OPEN", "InvalidStateError");
    }

    if (this[_sendQueue].length === 0) {
      // Fast path if the send queue is empty, for example when only synchronous
      // data is being sent.
      if (ArrayBufferIsView(data)) {
        op_ws_send_binary(this[_rid], data);
      } else if (isArrayBuffer(data)) {
        op_ws_send_binary_ab(this[_rid], data);
      } else if (ObjectPrototypeIsPrototypeOf(BlobPrototype, data)) {
        this[_queueSend](data);
      } else {
        const string = String(data);
        op_ws_send_text(
          this[_rid],
          string,
        );
      }
    } else {
      // Slower path if the send queue is not empty, for example when sending
      // asynchronous data like a Blob.
      this[_queueSend](data);
    }
  }

  close(code = undefined, reason = undefined) {
    webidl.assertBranded(this, WebSocketPrototype);
    const prefix = "Failed to execute 'close' on 'WebSocket'";

    if (code !== undefined) {
      code = webidl.converters["unsigned short"](code, prefix, "Argument 1", {
        clamp: true,
      });
    }

    if (reason !== undefined) {
      reason = webidl.converters.USVString(reason, prefix, "Argument 2");
    }

    if (!this[_server]) {
      if (
        code !== undefined &&
        !(code === 1000 || (3000 <= code && code < 5000))
      ) {
        throw new DOMException(
          `The close code must be either 1000 or in the range of 3000 to 4999: received ${code}`,
          "InvalidAccessError",
        );
      }
    }

    if (
      reason !== undefined &&
      TypedArrayPrototypeGetByteLength(core.encode(reason)) > 123
    ) {
      throw new DOMException(
        "The close reason may not be longer than 123 bytes",
        "SyntaxError",
      );
    }

    if (this[_readyState] === CONNECTING) {
      this[_readyState] = CLOSING;
    } else if (this[_readyState] === OPEN) {
      this[_readyState] = CLOSING;

      PromisePrototypeCatch(
        op_ws_close(
          this[_rid],
          code,
          reason,
        ),
        (err) => {
          this[_readyState] = CLOSED;

          const errorEv = new ErrorEvent("error", {
            error: err,
            message: ErrorPrototypeToString(err),
          });
          this.dispatchEvent(errorEv);

          const closeEv = new CloseEvent("close");
          this.dispatchEvent(closeEv);
          core.tryClose(this[_rid]);
        },
      );
    }
  }

  async [_eventLoop]() {
    const rid = this[_rid];
    while (this[_readyState] !== CLOSED) {
      const kind = await op_ws_next_event(rid);
      /* close the connection if read was cancelled, and we didn't get a close frame */
      if (
        (this[_readyState] == CLOSING) &&
        kind <= 3 && this[_role] !== CLIENT
      ) {
        this[_readyState] = CLOSED;

        const event = new CloseEvent("close");
        this.dispatchEvent(event);
        core.tryClose(rid);
        break;
      }

      switch (kind) {
        case 0: {
          /* string */
          const data = op_ws_get_buffer_as_string(rid);
          if (data === undefined) {
            break;
          }

          this[_serverHandleIdleTimeout]();
          const event = new MessageEvent("message", {
            data,
            origin: this[_url],
          });
          setIsTrusted(event, true);
          dispatch(this, event);
          break;
        }
        case 1: {
          /* binary */
          const d = op_ws_get_buffer(rid);
          if (d == undefined) {
            break;
          }

          this[_serverHandleIdleTimeout]();
          // deno-lint-ignore prefer-primordials
          const buffer = d.buffer;
          let data;
          if (this.binaryType === "blob") {
            data = new Blob([buffer]);
          } else {
            data = buffer;
          }

          const event = new MessageEvent("message", {
            data,
            origin: this[_url],
          });
          setIsTrusted(event, true);
          dispatch(this, event);
          break;
        }
        case 2: {
          /* pong */
          this[_serverHandleIdleTimeout]();
          break;
        }
        case 3: {
          /* error */
          this[_readyState] = CLOSED;

          const errorEv = new ErrorEvent("error", {
            message: op_ws_get_error(rid),
          });
          this.dispatchEvent(errorEv);

          const closeEv = new CloseEvent("close");
          this.dispatchEvent(closeEv);
          core.tryClose(rid);
          break;
        }
        default: {
          /* close */
          const code = kind;
          const reason = code == 1005 ? "" : op_ws_get_error(rid);
          const prevState = this[_readyState];
          this[_readyState] = CLOSED;
          clearTimeout(this[_idleTimeoutTimeout]);

          if (prevState === OPEN) {
            try {
              await op_ws_close(
                rid,
                code,
                reason,
              );
            } catch {
              // ignore failures
            }
          }

          const event = new CloseEvent("close", {
            wasClean: true,
            code: code,
            reason,
          });
          this.dispatchEvent(event);
          core.tryClose(rid);
          break;
        }
      }
    }
  }

  async [_queueSend](data) {
    const queue = this[_sendQueue];

    ArrayPrototypePush(queue, data);

    if (queue.length > 1) {
      // There is already a send in progress, so we just push to the queue
      // and let that task handle sending of this data.
      return;
    }

    while (queue.length > 0) {
      const data = queue[0];
      if (ArrayBufferIsView(data)) {
        op_ws_send_binary(this[_rid], data);
      } else if (isArrayBuffer(data)) {
        op_ws_send_binary_ab(this[_rid], data);
      } else if (ObjectPrototypeIsPrototypeOf(BlobPrototype, data)) {
        // deno-lint-ignore prefer-primordials
        const ab = await data.slice().arrayBuffer();
        op_ws_send_binary_ab(this[_rid], ab);
      } else {
        const string = String(data);
        op_ws_send_text(
          this[_rid],
          string,
        );
      }
      ArrayPrototypeShift(queue);
    }
  }

  [_serverHandleIdleTimeout]() {
    if (this[_idleTimeoutDuration]) {
      clearTimeout(this[_idleTimeoutTimeout]);
      this[_idleTimeoutTimeout] = setTimeout(async () => {
        if (this[_readyState] === OPEN) {
          await PromisePrototypeCatch(op_ws_send_ping(this[_rid]), () => {});
          this[_idleTimeoutTimeout] = setTimeout(async () => {
            if (this[_readyState] === OPEN) {
              this[_readyState] = CLOSING;
              const reason = "No response from ping frame.";
              await PromisePrototypeCatch(
                op_ws_close(this[_rid], 1001, reason),
                () => {},
              );
              this[_readyState] = CLOSED;

              const errEvent = new ErrorEvent("error", {
                message: reason,
              });
              this.dispatchEvent(errEvent);

              const event = new CloseEvent("close", {
                wasClean: false,
                code: 1001,
                reason,
              });
              this.dispatchEvent(event);
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

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(WebSocketPrototype, this),
        keys: [
          "url",
          "readyState",
          "extensions",
          "protocol",
          "binaryType",
          "bufferedAmount",
          "onmessage",
          "onerror",
          "onclose",
          "onopen",
        ],
      }),
      inspectOptions,
    );
  }
}

ObjectDefineProperties(WebSocket, {
  CONNECTING: {
    __proto__: null,
    value: 0,
  },
  OPEN: {
    __proto__: null,
    value: 1,
  },
  CLOSING: {
    __proto__: null,
    value: 2,
  },
  CLOSED: {
    __proto__: null,
    value: 3,
  },
});

defineEventHandler(WebSocket.prototype, "message");
defineEventHandler(WebSocket.prototype, "error");
defineEventHandler(WebSocket.prototype, "close");
defineEventHandler(WebSocket.prototype, "open");

webidl.configureInterface(WebSocket);
const WebSocketPrototype = WebSocket.prototype;

function createWebSocketBranded() {
  const socket = webidl.createBranded(WebSocket);
  socket[_rid] = undefined;
  socket[_role] = undefined;
  socket[_readyState] = CONNECTING;
  socket[_extensions] = "";
  socket[_protocol] = "";
  socket[_url] = "";
  // We use ArrayBuffer for server websockets for backwards compatibility
  // and performance reasons.
  //
  // https://github.com/denoland/deno/issues/15340#issuecomment-1872353134
  socket[_binaryType] = "arraybuffer";
  socket[_idleTimeoutDuration] = 0;
  socket[_idleTimeoutTimeout] = undefined;
  socket[_sendQueue] = [];
  return socket;
}

export {
  _eventLoop,
  _idleTimeoutDuration,
  _idleTimeoutTimeout,
  _protocol,
  _readyState,
  _rid,
  _role,
  _server,
  _serverHandleIdleTimeout,
  createWebSocketBranded,
  SERVER,
  WebSocket,
};
