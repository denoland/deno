// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync } = window.__bootstrap.dispatchJson;

  function metrics() {
    return sendSync("op_metrics");
  }

  window.__bootstrap.metrics = {
    metrics,
  };
})(this);
