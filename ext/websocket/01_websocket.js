// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file camelcase
/// <reference path="../../core/internal.d.ts" />

const core = globalThis.Deno.core;
import { URL } from "ext:deno_url/00_url.js";
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { isValidHTTPToken } from "ext:deno_web/00_infra.js";
import DOMException from "ext:deno_web/01_dom_exception.js";
import {
  _skipInternalInit,
  CloseEvent,
  defineEventHandler,
  dispatch,
  ErrorEvent,
  Event,
  EventTarget,
  MessageEvent,
} from "ext:deno_web/02_event.js";
import { Blob, BlobPrototype } from "ext:deno_web/09_file.js";
const primordials = globalThis.__bootstrap.primordials;
const {
  ArrayBufferPrototype,
  ArrayBufferIsView,
  ArrayBufferPrototypeGetByteLength,
  ArrayPrototypeJoin,
  ArrayPrototypeMap,
  ArrayPrototypeSome,
  DataView,
  DataViewPrototypeGetByteLength,
  ErrorPrototypeToString,
  ObjectDefineProperties,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  SafeSet,
  SetPrototypeGetSize,
  // TODO(lucacasonato): add SharedArrayBuffer to primordials
  // SharedArrayBufferPrototype
  String,
  StringPrototypeEndsWith,
  StringPrototypeToLowerCase,
  Symbol,
  SymbolIterator,
  PromisePrototypeCatch,
  SymbolFor,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetSymbolToStringTag,
} = primordials;
const op_ws_check_permission_and_cancel_handle =
  core.ops.op_ws_check_permission_and_cancel_handle;
const {
  op_ws_create,
  op_ws_close,
  op_ws_send_binary,
  op_ws_send_text,
  op_ws_next_event,
  op_ws_send_ping,
} = core.generateAsyncOpHandler(
  "op_ws_create",
  "op_ws_close",
  "op_ws_send_binary",
  "op_ws_send_text",
  "op_ws_next_event",
  "op_ws_send_ping",
);

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
    if (
      ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, V) ||
      // deno-lint-ignore prefer-primordials
      ObjectPrototypeIsPrototypeOf(SharedArrayBuffer.prototype, V)
    ) {
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
const _bufferedAmount = Symbol("[[bufferedAmount]]");
const _eventLoop = Symbol("[[eventLoop]]");

const _server = Symbol("[[server]]");
const _idleTimeoutDuration = Symbol("[[idleTimeout]]");
const _idleTimeoutTimeout = Symbol("[[idleTimeoutTimeout]]");
const _serverHandleIdleTimeout = Symbol("[[serverHandleIdleTimeout]]");
class WebSocket extends EventTarget {
  [_rid];
  [_role];

  [_readyState] = CONNECTING;
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

  [_extensions] = "";
  get extensions() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_extensions];
  }

  [_protocol] = "";
  get protocol() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_protocol];
  }

  [_url] = "";
  get url() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_url];
  }

  [_binaryType] = "blob";
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

  [_bufferedAmount] = 0;
  get bufferedAmount() {
    webidl.assertBranded(this, WebSocketPrototype);
    return this[_bufferedAmount];
  }

  constructor(url, protocols = []) {
    super();
    this[webidl.brand] = webidl.brand;
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
      wsURL = new URL(url);
    } catch (e) {
      throw new DOMException(e.message, "SyntaxError");
    }

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
        "Can't supply multiple times the same protocol.",
        "SyntaxError",
      );
    }

    if (
      ArrayPrototypeSome(
        protocols,
        (protocol) => isValidHTTPToken(protocol),
      )
    ) {
      throw new DOMException(
        "Invalid protocol value.",
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

  send(data) {
    webidl.assertBranded(this, WebSocketPrototype);
    const prefix = "Failed to execute 'send' on 'WebSocket'";

    webidl.requiredArguments(arguments.length, 1, prefix);
    data = webidl.converters.WebSocketSend(data, prefix, "Argument 1");

    if (this[_readyState] !== OPEN) {
      throw new DOMException("readyState not OPEN", "InvalidStateError");
    }

    /**
     * @param {ArrayBufferView} view
     * @param {number} byteLength
     */
    const sendTypedArray = (view, byteLength) => {
      this[_bufferedAmount] += byteLength;
      PromisePrototypeThen(
        op_ws_send_binary(
          this[_rid],
          view,
        ),
        () => {
          this[_bufferedAmount] -= byteLength;
        },
      );
    };

    if (ObjectPrototypeIsPrototypeOf(BlobPrototype, data)) {
      PromisePrototypeThen(
        // deno-lint-ignore prefer-primordials
        data.slice().arrayBuffer(),
        (ab) =>
          sendTypedArray(
            new DataView(ab),
            ArrayBufferPrototypeGetByteLength(ab),
          ),
      );
    } else if (ArrayBufferIsView(data)) {
      if (TypedArrayPrototypeGetSymbolToStringTag(data) === undefined) {
        // DataView
        sendTypedArray(data, DataViewPrototypeGetByteLength(data));
      } else {
        // TypedArray
        sendTypedArray(data, TypedArrayPrototypeGetByteLength(data));
      }
    } else if (ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, data)) {
      sendTypedArray(data, ArrayBufferPrototypeGetByteLength(data));
    } else {
      const string = String(data);
      const d = core.encode(string);
      this[_bufferedAmount] += TypedArrayPrototypeGetByteLength(d);
      PromisePrototypeThen(
        op_ws_send_text(
          this[_rid],
          string,
        ),
        () => {
          this[_bufferedAmount] -= TypedArrayPrototypeGetByteLength(d);
        },
      );
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
          "The close code must be either 1000 or in the range of 3000 to 4999.",
          "InvalidAccessError",
        );
      }
    }

    if (
      reason !== undefined &&
      TypedArrayPrototypeGetByteLength(core.encode(reason)) > 123
    ) {
      throw new DOMException(
        "The close reason may not be longer than 123 bytes.",
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
    while (this[_readyState] !== CLOSED) {
      const { 0: kind, 1: value } = await op_ws_next_event(this[_rid]);

      switch (kind) {
        case 0: {
          /* string */
          this[_serverHandleIdleTimeout]();
          const event = new MessageEvent("message", {
            data: value,
            origin: this[_url],
          });
          dispatch(this, event);
          break;
        }
        case 1: {
          /* binary */
          this[_serverHandleIdleTimeout]();
          let data;

          if (this.binaryType === "blob") {
            data = new Blob([value]);
          } else {
            data = value;
          }

          const event = new MessageEvent("message", {
            data,
            origin: this[_url],
            [_skipInternalInit]: true,
          });
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
            message: value,
          });
          this.dispatchEvent(errorEv);

          const closeEv = new CloseEvent("close");
          this.dispatchEvent(closeEv);
          core.tryClose(this[_rid]);
          break;
        }
        default: {
          /* close */
          const code = kind;
          const prevState = this[_readyState];
          this[_readyState] = CLOSED;
          clearTimeout(this[_idleTimeoutTimeout]);

          if (prevState === OPEN) {
            try {
              await op_ws_close(
                this[_rid],
                code,
                value,
              );
            } catch {
              // ignore failures
            }
          }

          const event = new CloseEvent("close", {
            wasClean: true,
            code: code,
            reason: value,
          });
          this.dispatchEvent(event);
          core.tryClose(this[_rid]);
          break;
        }
      }
    }
  }

  [_serverHandleIdleTimeout]() {
    if (this[_idleTimeoutDuration]) {
      clearTimeout(this[_idleTimeoutTimeout]);
      this[_idleTimeoutTimeout] = setTimeout(async () => {
        if (this[_readyState] === OPEN) {
          await op_ws_send_ping(this[_rid]);
          this[_idleTimeoutTimeout] = setTimeout(async () => {
            if (this[_readyState] === OPEN) {
              this[_readyState] = CLOSING;
              const reason = "No response from ping frame.";
              await op_ws_close(this[_rid], 1001, reason);
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

  [SymbolFor("Deno.customInspect")](inspect) {
    return `${this.constructor.name} ${
      inspect({
        url: this.url,
        readyState: this.readyState,
        extensions: this.extensions,
        protocol: this.protocol,
        binaryType: this.binaryType,
        bufferedAmount: this.bufferedAmount,
      })
    }`;
  }
}

ObjectDefineProperties(WebSocket, {
  CONNECTING: {
    value: 0,
  },
  OPEN: {
    value: 1,
  },
  CLOSING: {
    value: 2,
  },
  CLOSED: {
    value: 3,
  },
});

defineEventHandler(WebSocket.prototype, "message");
defineEventHandler(WebSocket.prototype, "error");
defineEventHandler(WebSocket.prototype, "close");
defineEventHandler(WebSocket.prototype, "open");

webidl.configurePrototype(WebSocket);
const WebSocketPrototype = WebSocket.prototype;

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
  SERVER,
  WebSocket,
};
