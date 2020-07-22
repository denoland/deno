// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const dispatchJson = window.__bootstrap.dispatchJson;

  function opNow() {
    return dispatchJson.sendSync("op_now");
  }

  window.__bootstrap.timers = { opNow };
})(this);
