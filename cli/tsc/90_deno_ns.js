// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports stable Deno APIs.

((window) => {
  window.__bootstrap.denoNs = {
    metrics: window.__bootstrap.metrics.metrics,
    version: window.__bootstrap.version.version,
    build: window.__bootstrap.build.build,
    errors: window.__bootstrap.errors.errors,
    customInspect: window.__bootstrap.console.customInspect,
    inspect: window.__bootstrap.console.inspect,
    resources: window.__bootstrap.resources.resources,
    close: window.__bootstrap.resources.close,
  };
})(this);
