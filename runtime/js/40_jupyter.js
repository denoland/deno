// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.__bootstrap.core;
  const jupyter = {};

  function display(mimeType, buf) {
    return core.opSync("op_jupyter_display", mimeType, buf);
  }

  jupyter.display = display;
  window.__bootstrap.jupyter = jupyter;
})(this);
