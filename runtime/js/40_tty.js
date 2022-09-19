// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;

  const size = new Uint32Array(2);
  const sizeU8 = new Uint8Array(8);
  function consoleSize(rid) {
    return ops.op_console_size(rid, sizeU8);
  }

  function isatty(rid) {
    return ops.op_isatty(rid);
  }

  const DEFAULT_SET_RAW_OPTIONS = {
    cbreak: false,
  };

  function setRaw(rid, mode, options = {}) {
    const rOptions = { ...DEFAULT_SET_RAW_OPTIONS, ...options };
    ops.op_set_raw({ rid, mode, options: rOptions });
  }

  window.__bootstrap.tty = {
    consoleSize,
    isatty,
    setRaw,
  };
})(this);
