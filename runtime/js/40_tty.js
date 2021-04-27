// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function consoleSize(rid) {
    return core.opSync("op_console_size", rid);
  }

  function isatty(rid) {
    return core.opSync("op_isatty", rid);
  }

  const DEFAULT_SET_RAW_OPTIONS = {
    cbreak: false,
  };

  function setRaw(rid, mode, options = {}) {
    const rOptions = { ...DEFAULT_SET_RAW_OPTIONS, ...options };
    core.opSync("op_set_raw", { rid, mode, options: rOptions });
  }

  window.__bootstrap.tty = {
    consoleSize,
    isatty,
    setRaw,
  };
})(this);
