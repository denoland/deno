// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendAsync } = window.__bootstrap.dispatchJson;

  class WebSocket extends EventTarget {
    constructor(url) {
      super();

      const wsURL = new URL(url);

      if ((wsURL.protocol !== "ws:" && wsURL.protocol !== "wss:") || wsURL.hash !== "") {
        throw new SyntaxError();
      }

      this.url = wsURL.href;

      sendAsync("op_ws_create", { url: wsURL.href }).then(rid => {
        this.rid = rid;
      });
    }

    close(code, reason) {
      if (code && (code !== 1000 && !(3000 <= code > 5000))) {
        throw new TypeError();
      }

      let encoder = new TextEncoder();
      if (reason && encoder.encode(reason).byteLength > 123) {
        throw new SyntaxError();
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
