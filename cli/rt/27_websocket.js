// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync, sendAsync } = window.__bootstrap.dispatchJson;

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

    constructor(url) {
      super();

      const wsURL = new URL(url);

      if (
        (wsURL.protocol !== "ws:" && wsURL.protocol !== "wss:") ||
        wsURL.hash !== ""
      ) {
        throw new DOMException("", "SyntaxError");
      }

      this.url = wsURL.href;

      sendAsync("op_ws_create", { url: wsURL.href }).then(({ type, rid }) => {
        if (type === "success") {
          this.rid = rid;
          this.readyState = this.OPEN;
          let event = new Event("open");
          event.target = this;
          this.onopen(event);
          this.dispatchEvent(event);
        } else {
          this.readyState = this.CLOSED;
          let event = new Event("error");
          event.target = this;
          this.onerror(event);
          this.dispatchEvent(event);
        }
      });
    }

    send(data) { // TODO: blob & arraybuffer
      sendSync("op_ws_send", {
        rid: this.rid,
        data,
      });
    }

    close(code, reason) {
      if (code && (code !== 1000 && !(3000 <= code > 5000))) {
        throw new DOMException("", "NotSupportedError");
      }

      let encoder = new TextEncoder();
      if (reason && encoder.encode(reason).byteLength > 123) {
        throw new DOMException("", "SyntaxError");
      }

      sendAsync("op_ws_close", {
        rid: this.rid,
        code,
        reason,
      }).then(() => {
        console.log("closed");
      });
    }
  }

  window.__bootstrap.webSocket = {
    WebSocket,
  };
})(this);
