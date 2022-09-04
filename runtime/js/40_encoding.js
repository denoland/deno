// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

"use strict";

((window) => {
  const { core } = window.__bootstrap;
  const { ops } = core;

  function escapeHtml(string) {
    return ops.op_escape_html(string);
  }

  window.__bootstrap.runtime_encoding = {
    escapeHtml,
  };
})(this);
