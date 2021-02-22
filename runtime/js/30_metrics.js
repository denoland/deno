// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function metrics() {
    const { combined, ops } = core.jsonOpSync("op_metrics");
    if (ops) {
      combined.ops = ops;
    }
    return combined;
  }

  window.__bootstrap.metrics = {
    metrics,
  };
})(this);
