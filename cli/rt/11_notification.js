// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  class Notification {
    constructor(title, options) {
      core.jsonOpSync("op_notify_send", { title, options });
    }
    // TODO(littledivy): How do we implement .close()?
    close() {}
  }
  window.__bootstrap.Notification = Notification;
})(this);
