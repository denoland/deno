// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  function requiredArguments(
    name,
    length,
    required,
  ) {
    if (length < required) {
      const errMsg = `${name} requires at least ${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present`;
      throw new TypeError(errMsg);
    }
  }

  window.__bootstrap.fetchUtil = {
    requiredArguments,
  };
})(this);
