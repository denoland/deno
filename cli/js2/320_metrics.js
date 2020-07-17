// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync } = window.__dispatchJson;

  function metrics() {
    return sendSync("op_metrics");
  }

  window.__metrics = {
    metrics,
  };
})(this);
