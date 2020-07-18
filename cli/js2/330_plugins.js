// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync } = window.__bootstrap.dispatchJson;

  function openPlugin(filename) {
    return sendSync("op_open_plugin", { filename });
  }

  window.__bootstrap.plugins = {
    openPlugin,
  };
})(this);
