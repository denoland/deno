// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync, sendAsync } = window.__bootstrap.dispatchJson;
  const { requiredArguments } = window.__bootstrap.webUtil;

  class WebSocket extends EventTarget {
    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;

    CONNECTING = 0;
    OPEN = 1;
    CLOSING = 2;
    CLOSED = 3;

    readyState = this.CONNECTING;

    onopen = () => {};
    onerror = () => {};
    onclose = () => {};

    constructor(url, protocols) {
      super();
      requiredArguments("WebSocket", arguments.length, 1);

      const wsURL = new URL(url);

      if (wsURL.protocol !== "ws:" && wsURL.protocol !== "wss:") {
        throw new DOMException(
          "Only ws & wss schemes are allowed in a WebSocket URL.",
          "SyntaxError",
        );
      }

      if (wsURL.hash !== "" && wsURL.href.endsWith("#")) {
        throw new DOMException(
          "Fragments are not allowed in a WebSocket URL.",
          "SyntaxError",
        );
      }

      this.url = wsURL.href;

      sendAsync("op_ws_create", { url: wsURL.href }).then(({ type, rid }) => {
        if (type === "success") {
          this.rid = rid;
          this.readyState = this.OPEN;
          const event = new Event("open");
          event.target = this;
          this.onopen(event);
          this.dispatchEvent(event);
        } else {
          this.readyState = this.CLOSED;
          const event = new Event("error");
          event.target = this;
          this.onerror(event);
          this.dispatchEvent(event);
        }
      });
    }

    send(data) {
      requiredArguments("WebSocket.send", arguments.length, 1);

      if (this.readyState !== this.CLOSING && this.readyState !== this.CLOSED) {
        if (data instanceof Blob) {
          data.arrayBuffer().then((buf) => {
            console.log(buf);
            sendSync("op_ws_send", {
              rid: this.rid,
            }, buf);
          });
        } else if (
          data instanceof Int8Array || data instanceof Int16Array ||
          data instanceof Int32Array || data instanceof Uint8Array ||
          data instanceof Uint16Array || data instanceof Uint32Array ||
          data instanceof Uint8ClampedArray || data instanceof Float32Array ||
          data instanceof Float64Array || data instanceof DataView
        ) {
          sendSync("op_ws_send", {
            rid: this.rid,
          }, data);
        } else if (data instanceof ArrayBuffer) { //TODO
          console.log(data);
          sendSync("op_ws_send", {
            rid: this.rid,
          }, data);
        } else {
          sendSync("op_ws_send", {
            rid: this.rid,
            text: String(data),
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

      this.readyState = this.CLOSING;

      sendAsync("op_ws_close", {
        rid: this.rid,
        code,
        reason,
      }).then(() => {
        this.readyState = this.CLOSED;
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
  }

  window.__bootstrap.webSocket = {
    WebSocket,
  };
})(this);
