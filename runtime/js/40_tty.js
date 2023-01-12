// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const {
    Uint32Array,
    Uint8Array,
  } = window.__bootstrap.primordials;
  const core = window.Deno.core;
  const ops = core.ops;

  const size = new Uint32Array(2);
  function consoleSize() {
    ops.op_console_size(size);
    return { columns: size[0], rows: size[1] };
  }

  const isattyBuffer = new Uint8Array(1);
  function isatty(rid) {
    ops.op_isatty(rid, isattyBuffer);
    return !!isattyBuffer[0];
  }

  window.__bootstrap.tty = {
    consoleSize,
    isatty,
  };
})(this);
