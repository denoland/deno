// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function dlopen(path) {
    return core.opSync("op_dlopen", path);
  }

  function dlcall(rid, args) {
    return core.opSync("op_dlcall", rid, args);
  }

  window.__bootstrap.ffi = {
    dlopen,
    dlcall,
  };
})(this);
