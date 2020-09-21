// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  function openPlugin(filename) {
    return core.jsonOpSync("op_open_plugin", { filename });
  }

  window.__bootstrap.plugins = {
    openPlugin,
  };
})(this);
