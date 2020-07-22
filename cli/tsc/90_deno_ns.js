// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports stable Deno APIs.

((window) => {
  window.__bootstrap.denoNs = {
    build: window.__bootstrap.build.build,
    errors: window.__bootstrap.errors.errors,
  };
})(this);
