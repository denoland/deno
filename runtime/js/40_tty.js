// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;

  function consoleSize(rid) {
    return ops.op_console_size(rid);
  }

  function isatty(rid) {
    return ops.op_isatty(rid);
  }

  window.__bootstrap.tty = {
    consoleSize,
    isatty,
  };
})(this);
