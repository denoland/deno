// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const { URL } = window.__bootstrap.url;
  const webidl = window.__bootstrap.webidl;
  const { HTTP_TOKEN_CODE_POINT_RE } = window.__bootstrap.infra;
  const { DOMException } = window.__bootstrap.domException;

  webidl.converters["sequence<DOMString> or DOMString"] = (V, opts) => {
    // Union for (sequence<DOMString> or DOMString)
    if (webidl.type(V) === "Object" && V !== null) {
      if (V[Symbol.iterator] !== undefined) {
        return webidl.converters["sequence<DOMString>"](V, opts);
      }
    }
    return webidl.converters.DOMString(V, opts);
  };

  webidl.converters["WebSocketSend"] = (V, opts) => {
    // Union for (Blob or ArrayBufferView or ArrayBuffer or USVString)
    if (V instanceof Blob) {
      return webidl.converters["Blob"](V, opts);
    }
    if (typeof V === "object") {
      if (V instanceof ArrayBuffer || V instanceof SharedArrayBuffer) {
        return webidl.converters["ArrayBuffer"](V, opts);
      }
      if (ArrayBuffer.isView(V)) {
        return webidl.converters["ArrayBufferView"](V, opts);
      }
    }
    return webidl.converters["USVString"](V, opts);
  };

  const CONNECTING = 0;
  const OPEN = 1;
  const CLOSING = 2;
  const CLOSED = 3;

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

  const handlerSymbol = Symbol("eventHandlers");
  function makeWrappedHandler(handler) {
    function wrappedHandler(...args) {
      if (typeof wrappedHandler.handler !== "function") {
        return;
      }
      return wrappedHandler.handler.call(this, ...args);
    }
    wrappedHandler.handler = handler;
    return wrappedHandler;
  }
  // TODO(lucacasonato) reuse when we can reuse code between web crates
  function defineEventHandler(emitter, name) {
    // HTML specification section 8.1.5.1
    Object.defineProperty(emitter, `on${name}`, {
      get() {
        return this[handlerSymbol]?.get(name)?.handler;
      },
      set(value) {
        if (!this[handlerSymbol]) {
          this[handlerSymbol] = new Map();
        }
        let handlerWrapper = this[handlerSymbol]?.get(name);
        if (handlerWrapper) {
          handlerWrapper.handler = value;
        } else {
          handlerWrapper = makeWrappedHandler(value);
          this.addEventListener(name, handlerWrapper);
        }
        this[handlerSymbol].set(name, handlerWrapper);
      },
      configurable: true,
      enumerable: true,
    });
  }

  const _readyState = Symbol("[[readyState]]");
  const _url = Symbol("[[url]]");
  const _rid = Symbol("[[rid]]");
  const _extensions = Symbol("[[extensions]]");
  const _protocol = Symbol("[[protocol]]");
  const _binaryType = Symbol("[[binaryType]]");
  const _bufferedAmount = Symbol("[[bufferedAmount]]");
  class WebSocket extends EventTarget {
    [_rid];

    [_readyState] = CONNECTING;
    get readyState() {
      webidl.assertBranded(this, WebSocket);
      return this[_readyState];
    }

    get CONNECTING() {
      webidl.assertBranded(this, WebSocket);
      return CONNECTING;
    }
    get OPEN() {
      webidl.assertBranded(this, WebSocket);
      return OPEN;
    }
    get CLOSING() {
      webidl.assertBranded(this, WebSocket);
      return CLOSING;
    }
    get CLOSED() {
      webidl.assertBranded(this, WebSocket);
      return CLOSED;
    }

    [_extensions] = "";
    get extensions() {
      webidl.assertBranded(this, WebSocket);
      return this[_extensions];
    }

    [_protocol] = "";
    get protocol() {
      webidl.assertBranded(this, WebSocket);
      return this[_protocol];
    }

    [_url] = "";
    get url() {
      webidl.assertBranded(this, WebSocket);
      return this[_url];
    }

    [_binaryType] = "blob";
    get binaryType() {
      webidl.assertBranded(this, WebSocket);
      return this[_binaryType];
    }
    set binaryType(value) {
      webidl.assertBranded(this, WebSocket);
      value = webidl.converters.DOMString(value, {
        prefix: "Failed to set 'binaryType' on 'WebSocket'",
      });
      if (value === "blob" || value === "arraybuffer") {
        this[_binaryType] = value;
      }
    }

    [_bufferedAmount] = 0;
    get bufferedAmount() {
      webidl.assertBranded(this, WebSocket);
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

      if (wsURL.hash !== "" || wsURL.href.endsWith("#")) {
        throw new DOMException(
          "Fragments are not allowed in a WebSocket URL.",
          "SyntaxError",
        );
      }

      this[_url] = wsURL.href;

      core.opSync("op_ws_check_permission", this[_url]);

      if (typeof protocols === "string") {
        protocols = [protocols];
      }

      if (
        protocols.length !== new Set(protocols.map((p) => p.toLowerCase())).size
      ) {
        throw new DOMException(
          "Can't supply multiple times the same protocol.",
          "SyntaxError",
        );
      }

      if (
        protocols.some((protocol) => !HTTP_TOKEN_CODE_POINT_RE.test(protocol))
      ) {
        throw new DOMException(
          "Invalid protocol value.",
          "SyntaxError",
        );
      }

      core.opAsync("op_ws_create", {
        url: wsURL.href,
        protocols: protocols.join(", "),
      }).then((create) => {
        this[_rid] = create.rid;
        this[_extensions] = create.extensions;
        this[_protocol] = create.protocol;

        if (this[_readyState] === CLOSING) {
          core.opAsync("op_ws_close", {
            rid: this[_rid],
          }).then(() => {
            this[_readyState] = CLOSED;

            const errEvent = new ErrorEvent("error");
            this.dispatchEvent(errEvent);

            const event = new CloseEvent("close");
            this.dispatchEvent(event);
            tryClose(this[_rid]);
          });
        } else {
          this[_readyState] = OPEN;
          const event = new Event("open");
          this.dispatchEvent(event);

          this.#eventLoop();
        }
      }).catch((err) => {
        this[_readyState] = CLOSED;

        const errorEv = new ErrorEvent(
          "error",
          { error: err, message: err.toString() },
        );
        this.dispatchEvent(errorEv);

        const closeEv = new CloseEvent("close");
        this.dispatchEvent(closeEv);
      });
    }

    send(data) {
      webidl.assertBranded(this, WebSocket);
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
        core.opAsync("op_ws_send", {
          rid: this[_rid],
          kind: "binary",
        }, ta).then(() => {
          this[_bufferedAmount] -= ta.byteLength;
        });
      };

      if (data instanceof Blob) {
        data.slice().arrayBuffer().then((ab) =>
          sendTypedArray(new DataView(ab))
        );
      } else if (ArrayBuffer.isView(data)) {
        sendTypedArray(data);
      } else if (data instanceof ArrayBuffer) {
        sendTypedArray(new DataView(data));
      } else {
        const string = String(data);
        const d = core.encode(string);
        this[_bufferedAmount] += d.byteLength;
        core.opAsync("op_ws_send", {
          rid: this[_rid],
          kind: "text",
          text: string,
        }).then(() => {
          this[_bufferedAmount] -= d.byteLength;
        });
      }
    }

    close(code = undefined, reason = undefined) {
      webidl.assertBranded(this, WebSocket);
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

      if (
        code !== undefined && !(code === 1000 || (3000 <= code && code < 5000))
      ) {
        throw new DOMException(
          "The close code must be either 1000 or in the range of 3000 to 4999.",
          "InvalidAccessError",
        );
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

        core.opAsync("op_ws_close", {
          rid: this[_rid],
          code,
          reason,
        }).then(() => {
          this[_readyState] = CLOSED;
          const event = new CloseEvent("close", {
            wasClean: true,
            code: code ?? 1005,
            reason,
          });
          this.dispatchEvent(event);
          tryClose(this[_rid]);
        });
      }
    }

    async #eventLoop() {
      while (this[_readyState] === OPEN) {
        const { kind, value } = await core.opAsync(
          "op_ws_next_event",
          this[_rid],
        );

        switch (kind) {
          case "string": {
            const event = new MessageEvent("message", {
              data: value,
              origin: this[_url],
            });
            this.dispatchEvent(event);
            break;
          }
          case "binary": {
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
            core.opAsync("op_ws_send", {
              rid: this[_rid],
              kind: "pong",
            });
            break;
          }
          case "close": {
            this[_readyState] = CLOSED;

            const event = new CloseEvent("close", {
              wasClean: true,
              code: value.code,
              reason: value.reason,
            });
            this.dispatchEvent(event);
            tryClose(this[_rid]);
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
            tryClose(this[_rid]);
            break;
          }
        }
      }
    }
  }

  Object.defineProperties(WebSocket, {
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

  window.__bootstrap.webSocket = { WebSocket };
})(this);
