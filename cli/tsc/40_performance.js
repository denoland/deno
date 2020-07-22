// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const dispatchJson = window.__bootstrap.dispatchJson;

  function now() {
    const res = dispatchJson.sendSync("op_now");
    return res.seconds * 1e3 + res.subsecNanos / 1e6;
  }

  class Performance {
    now() {
      return now();
    }
  }

  window.__bootstrap.performance = {
    Performance,
  };
})(this);
