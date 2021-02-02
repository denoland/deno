"use strict";
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  function metrics() {
    return core.jsonOpSync("op_metrics");
  }

  window.__bootstrap.metrics = {
    metrics,
  };
})(this);
