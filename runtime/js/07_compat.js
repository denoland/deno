// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const {
    Error,
  } = window.__bootstrap.primordials;

  class ERR_MODULE_NOT_FOUND extends Error {
    constructor(msg) {
      super(msg);
      this.code = "ERR_MODULE_NOT_FOUND";
    }
  }

  const errors = {
    ERR_MODULE_NOT_FOUND,
  };

  window.__bootstrap.compat = {
    errors,
  };
})(this);
