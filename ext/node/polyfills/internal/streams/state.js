// deno-lint-ignore-file
// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { validateInteger } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const _mod1 = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { ERR_INVALID_ARG_VALUE } = _mod1.codes;

const {
  MathFloor,
  NumberIsInteger,
} = primordials;

const lazyProcess = core.createLazyLoader("node:process");

// TODO (fix): For some reason Windows CI fails with bigger hwm.
let defaultHighWaterMarkBytes;
let defaultHighWaterMarkObjectMode = 16;

function ensureDefaults() {
  if (defaultHighWaterMarkBytes === undefined) {
    defaultHighWaterMarkBytes = lazyProcess().platform === "win32"
      ? 16 * 1024
      : 64 * 1024;
  }
}

function highWaterMarkFrom(options, isDuplex, duplexKey) {
  return options.highWaterMark != null
    ? options.highWaterMark
    : isDuplex
    ? options[duplexKey]
    : null;
}

function getDefaultHighWaterMark(objectMode) {
  ensureDefaults();
  return objectMode
    ? defaultHighWaterMarkObjectMode
    : defaultHighWaterMarkBytes;
}

function setDefaultHighWaterMark(objectMode, value) {
  validateInteger(value, "value", 0);
  if (objectMode) {
    defaultHighWaterMarkObjectMode = value;
  } else {
    defaultHighWaterMarkBytes = value;
  }
}

function getHighWaterMark(state, options, duplexKey, isDuplex) {
  const hwm = highWaterMarkFrom(options, isDuplex, duplexKey);
  if (hwm != null) {
    if (!NumberIsInteger(hwm) || hwm < 0) {
      const name = isDuplex ? `options.${duplexKey}` : "options.highWaterMark";
      throw new ERR_INVALID_ARG_VALUE(name, hwm);
    }
    return MathFloor(hwm);
  }

  // Default value
  return getDefaultHighWaterMark(state.objectMode);
}

return {
  getHighWaterMark,
  getDefaultHighWaterMark,
  setDefaultHighWaterMark,
  default: {
    getHighWaterMark,
    getDefaultHighWaterMark,
    setDefaultHighWaterMark,
  },
};
})();
