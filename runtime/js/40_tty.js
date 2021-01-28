// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  function consoleSize(rid) {
    return core.jsonOpSync("op_console_size", { rid });
  }

  function isatty(rid) {
    return core.jsonOpSync("op_isatty", { rid });
  }

  const DEFAULT_SET_RAW_OPTIONS = {
    cbreak: false,
  };

  function setRaw(rid, mode, options = {}) {
    const rOptions = { ...DEFAULT_SET_RAW_OPTIONS, ...options };
    core.jsonOpSync("op_set_raw", { rid, mode, options: rOptions });
  }

  window.__bootstrap.tty = {
    consoleSize,
    isatty,
    setRaw,
  };
})(this);
