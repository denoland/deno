// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync } = window.__bootstrap.dispatchJson;

  function consoleSize(rid) {
    return sendSync("op_console_size", { rid });
  }

  function isatty(rid) {
    return sendSync("op_isatty", { rid });
  }

  function setRaw(rid, mode) {
    sendSync("op_set_raw", { rid, mode });
  }

  window.__bootstrap.tty = {
    consoleSize,
    isatty,
    setRaw,
  };
})(this);
