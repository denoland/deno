// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
import { op_bootstrap_log_level } from "ext:core/ops";
const { SafeArrayIterator } = primordials;

// WARNING: Keep this in sync with Rust (search for LogLevel)
const LogLevel = {
  Error: 1,
  Warn: 2,
  Info: 3,
  Debug: 4,
};

const logSource = "JS";

let logLevel_ = null;
function logLevel() {
  if (logLevel_ === null) {
    logLevel_ = op_bootstrap_log_level() || 3;
  }
  return logLevel_;
}

function log(...args) {
  if (logLevel() >= LogLevel.Debug) {
    // if we destructure `console` off `globalThis` too early, we don't bind to
    // the right console, therefore we don't log anything out.
    globalThis.console.error(
      `DEBUG ${logSource} -`,
      ...new SafeArrayIterator(args),
    );
  }
}

export { log };
