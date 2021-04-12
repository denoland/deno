// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function metrics() {
    const { combined, ops } = core.opSync("op_metrics");
    if (ops) {
      // Re-map array of op metrics to be keyed by name
      const opPairs = Object.entries(core.opsCache);
      combined.ops = opPairs.reduce((accu, [name, id]) => {
        accu[name] = ops[id];
        return accu;
      }, {});
    }
    return combined;
  }

  window.__bootstrap.metrics = {
    metrics,
  };
})(this);
