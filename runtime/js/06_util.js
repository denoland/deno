// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { primordials } = globalThis.__bootstrap;
const { op_bootstrap_log_level } = globalThis.__bootstrap.core.ops;
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

return { log };
})();
