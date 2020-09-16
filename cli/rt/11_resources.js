// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;

  function resources() {
    const res = core.jsonOpSync("op_resources");
    const resources = {};
    for (const resourceTuple of res) {
      resources[resourceTuple[0]] = resourceTuple[1];
    }
    return resources;
  }

  function close(rid) {
    core.jsonOpSync("op_close", { rid });
  }

  window.__bootstrap.resources = {
    close,
    resources,
  };
})(this);
