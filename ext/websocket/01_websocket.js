// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

/// <reference path="../../core/internal.d.ts" />

((window) => {
  const core = window.Deno.core;
  const { URL } = window.__bootstrap.url;
  const webidl = window.__bootstrap.webidl;
  const { HTTP_TOKEN_CODE_POINT_RE } = window.__bootstrap.infra;
  const { DOMException } = window.__bootstrap.domException;
  const { defineEventHandler } = window.__bootstrap.event;
  const { Blob, BlobPrototype } = globalThis.__bootstrap.file;
  const {
    ArrayBufferPrototype,
    ArrayBufferIsView,
    ArrayPrototypeJoin,
    ArrayPrototypeMap,
    ArrayPrototypeSome,
    DataView,
    ErrorPrototypeToString,
    ObjectDefineProperties,
    ObjectPrototypeIsPrototypeOf,
    PromisePrototypeThen,
    RegExpPrototypeTest,
    Set,
    String,
    StringPrototypeEndsWith,
    StringPrototypeToLowerCase,
    Symbol,
    SymbolIterator,
    PromisePrototypeCatch,
    SymbolFor,
  } = window.__bootstrap.primordials;

  webidl.converters["sequence<DOMString> or DOMString"] = (V, opts) => {
    // Union for (sequence<DOMString> or DOMString)
    if (webidl.type(V) === "Object" && V !== null) {
      if (V[SymbolIterator] !== undefined) {
        return webidl.converters["sequence<DOMString>"](V, opts);
      }
    }
    return webidl.converters.DOMString(V, opts);
  };

  webidl.converters["WebSocketSend"] = (V, opts) => {
    // Union for (Blob or ArrayBufferView or ArrayBuffer or USVString)
    if (ObjectPrototypeIsPrototypeOf(BlobPrototype, V)) {
      return webidl.converters["Blob"](V, opts);
    }
    if (typeof V === "object") {
      // TODO(littledivy): use primordial for SharedArrayBuffer
      if (
        ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, V) ||
        ObjectPrototypeIsPrototypeOf(SharedArrayBuffer.prototype, V)
      ) {
        return webidl.converters["ArrayBuffer"](V, opts);
      }
      if (ArrayBufferIsView(V)) {
        return webidl.converters["ArrayBufferView"](V, opts);
      }
    }
    return webidl.converters["USVString"](V, opts);
  };

  const CONNECTING = 0;
  const OPEN = 1;
  const CLOSING = 2;
  const CLOSED = 3;

  const _readyState = Symbol("[[readyState]]");
  const _url = Symbol("[[url]]");
  const _rid = Symbol("[[rid]]");
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
      value = webidl.converters.DOMString(value, {
        prefix: "Failed to set 'binaryType' on 'WebSocket'",
      });
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
      webidl.requiredArguments(arguments.length, 1, {
        prefix,
      });
      url = webidl.converters.USVString(url, {
        prefix,
        context: "Argument 1",
      });
      protocols = webidl.converters["sequence<DOMString> or DOMString"](
        protocols,
        {
          prefix,
          context: "Argument 2",
        },
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

      core.opSync(
        "op_ws_check_permission_and_cancel_handle",
        this[_url],
        false,
      );

      if (typeof protocols === "string") {
        protocols = [protocols];
      }

      if (
        protocols.length !==
          new Set(
            ArrayPrototypeMap(protocols, (p) => StringPrototypeToLowerCase(p)),
          ).size
      ) {
        throw new DOMException(
          "Can't supply multiple times the same protocol.",
          "SyntaxError",
        );
      }

      if (
        ArrayPrototypeSome(
          protocols,
          (protocol) =>
            !RegExpPrototypeTest(HTTP_TOKEN_CODE_POINT_RE, protocol),
        )
      ) {
        throw new DOMException(
          "Invalid protocol value.",
          "SyntaxError",
        );
      }

      PromisePrototypeThen(
        core.opAsync(
          "op_ws_create",
          wsURL.href,
          ArrayPrototypeJoin(protocols, ", "),
        ),
        (create) => {
          this[_rid] = create.rid;
          this[_extensions] = create.extensions;
          this[_protocol] = create.protocol;

          if (this[_readyState] === CLOSING) {
            PromisePrototypeThen(
              core.opAsync("op_ws_close", this[_rid]),
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

      webidl.requiredArguments(arguments.length, 1, {
        prefix,
      });
      data = webidl.converters.WebSocketSend(data, {
        prefix,
        context: "Argument 1",
      });

      if (this[_readyState] !== OPEN) {
        throw new DOMException("readyState not OPEN", "InvalidStateError");
      }

      const sendTypedArray = (ta) => {
        this[_bufferedAmount] += ta.byteLength;
        PromisePrototypeThen(
          core.opAsync("op_ws_send", this[_rid], {
            kind: "binary",
            value: ta,
          }),
          () => {
            this[_bufferedAmount] -= ta.byteLength;
          },
        );
      };

      if (ObjectPrototypeIsPrototypeOf(BlobPrototype, data)) {
        PromisePrototypeThen(
          data.slice().arrayBuffer(),
          (ab) => sendTypedArray(new DataView(ab)),
        );
      } else if (ArrayBufferIsView(data)) {
        sendTypedArray(data);
      } else if (ObjectPrototypeIsPrototypeOf(ArrayBufferPrototype, data)) {
        sendTypedArray(new DataView(data));
      } else {
        const string = String(data);
        const d = core.encode(string);
        this[_bufferedAmount] += d.byteLength;
        PromisePrototypeThen(
          core.opAsync("op_ws_send", this[_rid], {
            kind: "text",
            value: string,
          }),
          () => {
            this[_bufferedAmount] -= d.byteLength;
          },
        );
      }
    }

    close(code = undefined, reason = undefined) {
      webidl.assertBranded(this, WebSocketPrototype);
      const prefix = "Failed to execute 'close' on 'WebSocket'";

      if (code !== undefined) {
        code = webidl.converters["unsigned short"](code, {
          prefix,
          clamp: true,
          context: "Argument 1",
        });
      }

      if (reason !== undefined) {
        reason = webidl.converters.USVString(reason, {
          prefix,
          context: "Argument 2",
        });
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

      if (reason !== undefined && core.encode(reason).byteLength > 123) {
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
          core.opAsync("op_ws_close", this[_rid], code, reason),
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
        const { kind, value } = await core.opAsync(
          "op_ws_next_event",
          this[_rid],
        );

        switch (kind) {
          case "string": {
            this[_serverHandleIdleTimeout]();
            const event = new MessageEvent("message", {
              data: value,
              origin: this[_url],
            });
            this.dispatchEvent(event);
            break;
          }
          case "binary": {
            this[_serverHandleIdleTimeout]();
            let data;

            if (this.binaryType === "blob") {
              data = new Blob([value]);
            } else {
              data = value.buffer;
            }

            const event = new MessageEvent("message", {
              data,
              origin: this[_url],
            });
            this.dispatchEvent(event);
            break;
          }
          case "ping": {
            core.opAsync("op_ws_send", this[_rid], {
              kind: "pong",
            });
            break;
          }
          case "pong": {
            this[_serverHandleIdleTimeout]();
            break;
          }
          case "closed":
          case "close": {
            const prevState = this[_readyState];
            this[_readyState] = CLOSED;
            clearTimeout(this[_idleTimeoutTimeout]);

            if (prevState === OPEN) {
              try {
                await core.opAsync(
                  "op_ws_close",
                  this[_rid],
                  value.code,
                  value.reason,
                );
              } catch {
                // ignore failures
              }
            }

            const event = new CloseEvent("close", {
              wasClean: true,
              code: value.code,
              reason: value.reason,
            });
            this.dispatchEvent(event);
            core.tryClose(this[_rid]);
            break;
          }
          case "error": {
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
        }
      }
    }

    [_serverHandleIdleTimeout]() {
      if (this[_idleTimeoutDuration]) {
        clearTimeout(this[_idleTimeoutTimeout]);
        this[_idleTimeoutTimeout] = setTimeout(async () => {
          if (this[_readyState] === OPEN) {
            await core.opAsync("op_ws_send", this[_rid], {
              kind: "ping",
            });
            this[_idleTimeoutTimeout] = setTimeout(async () => {
              if (this[_readyState] === OPEN) {
                this[_readyState] = CLOSING;
                const reason = "No response from ping frame.";
                await core.opAsync("op_ws_close", this[_rid], 1001, reason);
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

  window.__bootstrap.webSocket = {
    WebSocket,
    _rid,
    _readyState,
    _eventLoop,
    _protocol,
    _server,
    _idleTimeoutDuration,
    _idleTimeoutTimeout,
    _serverHandleIdleTimeout,
  };
})(this);
