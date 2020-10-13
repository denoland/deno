// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
    const core = window.Deno.core;  
    function Notification(title, message) {
      core.jsonOpSync("op_notify_send", { title, message });
    }
  
    window.__bootstrap.Notification = Notification;
  })(this);
  