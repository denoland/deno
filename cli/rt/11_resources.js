// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const sendSync = window.__bootstrap.dispatchJson.sendSync;

  function resources() {
    const res = sendSync("op_resources");
    const resources = {};
    for (const resourceTuple of res) {
      resources[resourceTuple[0]] = resourceTuple[1];
    }
    return resources;
  }

  function close(rid) {
    sendSync("op_close", { rid });
  }

  window.__bootstrap.resources = {
    close,
    resources,
  };
})(this);
