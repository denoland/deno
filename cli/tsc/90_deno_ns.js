// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This module exports stable Deno APIs.

((window) => {
  window.__bootstrap.denoNs = {
    test: window.__bootstrap.testing.test,
    metrics: window.__bootstrap.metrics.metrics,
    cwd: window.__bootstrap.fs.cwd,
    version: window.__bootstrap.version.version,
    build: window.__bootstrap.build.build,
    statSync: window.__bootstrap.fs.statSync,
    lstatSync: window.__bootstrap.fs.lstatSync,
    stat: window.__bootstrap.fs.stat,
    lstat: window.__bootstrap.fs.lstat,
    truncateSync: window.__bootstrap.fs.truncateSync,
    truncate: window.__bootstrap.fs.truncate,
    errors: window.__bootstrap.errors.errors,
    customInspect: window.__bootstrap.console.customInspect,
    inspect: window.__bootstrap.console.inspect,
    env: window.__bootstrap.os.env,
    exit: window.__bootstrap.os.exit,
    execPath: window.__bootstrap.os.execPath,
    resources: window.__bootstrap.resources.resources,
    close: window.__bootstrap.resources.close,
  };
})(this);
