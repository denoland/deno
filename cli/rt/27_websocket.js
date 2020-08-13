// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync, sendAsync } = window.__bootstrap.dispatchJson;
  const { requiredArguments } = window.__bootstrap.webUtil;

  class WebSocket extends EventTarget {
    #CONNECTING = 0;
    #OPEN = 1;
    #CLOSING = 2;
    #CLOSED = 3;

    get CONNECTING() {
      return this.#CONNECTING;
    }
    get OPEN() {
      return this.#OPEN;
    }
    get CLOSING() {
      return this.#CLOSING;
    }
    get CLOSED() {
      return this.#CLOSED;
    }

    #readyState = this.#CONNECTING;
    get readyState() {
      return this.#readyState;
    }

    #extensions = "";
    #protocol = "";
    #url = "";
    #rid = -1;

    get extensions() {
      return this.#extensions;
    }
    get protocol() {
      return this.#protocol;
    }

    binaryType = "blob";
    #bufferedAmount = 0;
    get bufferedAmount() {
      return this.#bufferedAmount;
    }

    get url() {
      return this.#url;
    }

    onopen = () => {};
    onerror = () => {};
    onclose = () => {};
    onmessage = () => {};

    constructor(url, protocols = []) {
      super();
      requiredArguments("WebSocket", arguments.length, 1);

      const wsURL = new URL(url);

      if (wsURL.protocol !== "ws:" && wsURL.protocol !== "wss:") {
        throw new DOMException(
          "Only ws & wss schemes are allowed in a WebSocket URL.",
          "SyntaxError",
        );
      }

      if (wsURL.hash !== "" && !wsURL.href.endsWith("#")) {
        throw new DOMException(
          "Fragments are not allowed in a WebSocket URL.",
          "SyntaxError",
        );
      }

      this.#url = wsURL.href;

      if (protocols && typeof protocols === "string") {
        protocols = [protocols];
      }

      sendAsync("op_ws_create", {
        url: wsURL.href,
        protocols: protocols.join("; "),
      }).then((create) => {
        if (create.success) {
          this.#rid = create.rid;
          this.#extensions = create.extensions;
          this.#protocol = create.protocol;
          this.#readyState = this.#OPEN;
          const event = new Event("open");
          event.target = this;
          this.onopen(event);
          this.dispatchEvent(event);

          this.#eventLoop();
        } else {
          this.#readyState = this.#CLOSED;
          const event = new Event("error");
          event.target = this;
          this.onerror(event);
          this.dispatchEvent(event);
        }
      });
    }

    send(data) {
      requiredArguments("WebSocket.send", arguments.length, 1);

      if (
        this.#readyState !== this.#CLOSING && this.#readyState !== this.#CLOSED
      ) {
        if (data instanceof Blob) {
          data.arrayBuffer().then((buf) => {
            this.#bufferedAmount += buf.byteLength;
            sendSync("op_ws_send", {
              rid: this.#rid,
            }, buf);
          });
        } else if (
          data instanceof Int8Array || data instanceof Int16Array ||
          data instanceof Int32Array || data instanceof Uint8Array ||
          data instanceof Uint16Array || data instanceof Uint32Array ||
          data instanceof Uint8ClampedArray || data instanceof Float32Array ||
          data instanceof Float64Array || data instanceof DataView
        ) {
          this.#bufferedAmount += data.byteLength;
          sendSync("op_ws_send", {
            rid: this.#rid,
          }, data);
        } else if (data instanceof ArrayBuffer) { //TODO
          this.#bufferedAmount += data.byteLength;
          sendSync("op_ws_send", {
            rid: this.#rid,
          }, data);
        } else {
          const string = String(data);
          const encoder = new TextEncoder();
          this.#bufferedAmount += encoder.encode(string).byteLength;
          sendSync("op_ws_send", {
            rid: this.#rid,
            text: string,
          });
        }
      } else {
        const event = new Event("error");
        event.target = this;
        this.onerror(event);
        this.dispatchEvent(event);
      }
    }

    close(code, reason) {
      if (code && (code !== 1000 && !(3000 <= code > 5000))) {
        throw new DOMException(
          "The close code must be either 1000 or in the range of 3000 to 4999.",
          "NotSupportedError",
        );
      }

      const encoder = new TextEncoder();
      if (reason && encoder.encode(reason).byteLength > 123) {
        throw new DOMException(
          "The close reason may not be longer than 123 bytes.",
          "SyntaxError",
        );
      }

      this.#readyState = this.#CLOSING;

      sendAsync("op_ws_close", {
        rid: this.#rid,
        code,
        reason,
      }).then(() => {
        this.#readyState = this.#CLOSED;
        const event = new CloseEvent("close", {
          wasClean: true,
          code,
          reason,
        });
        event.target = this;
        this.onclose(event);
        this.dispatchEvent(event);
      });
    }

async #eventLoop() {
      const message = await sendAsync("op_ws_next_event", { rid: this.#rid });
      if (message.type === "string" || message.type === "binary") {
        let data;

        if (message.type === "string") {
          data = message.data;
        } else {
          if (this.binaryType === "blob") {
            data = new Blob([message.data]);
          } else {
            data = message.data;
          }
        }

        const event = new MessageEvent("message", {
          data,
          origin: this.#url,
        });
        event.target = this;
        this.onmessage(event);
        this.dispatchEvent(event);

        this.#eventLoop();
      } else if (message.type === "close") {
        this.#readyState = this.#CLOSED;
        const event = new CloseEvent("close", {
          wasClean: true,
          code: message.code,
          reason: message.reason,
        });
        event.target = this;
        this.onclose(event);
        this.dispatchEvent(event);
      } else if (message.type === "error") {
        const event = new Event("error");
        event.target = this;
        this.onerror(event);
        this.dispatchEvent(event);
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

  window.__bootstrap.webSocket = {
    WebSocket,
  };
})(this);
