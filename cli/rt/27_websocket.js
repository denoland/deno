// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync } = window.__bootstrap.dispatchJson;

  class WebSocket extends EventTarget {
    constructor(url) {
      super();

      this.rid = sendSync("op_ws_create", { url });
    }

    close(code, reason) {
      sendSync("op_ws_close", {
        streamRid: this.rid,
        code,
        reason,
      });
    }
  }

  window.__bootstrap.webSocket = {
    WebSocket,
  };
})(this);
