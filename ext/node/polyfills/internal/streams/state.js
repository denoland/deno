// deno-lint-ignore-file
// Copyright 2018-2025 the Deno authors. MIT license.

import process from "node:process";
import { primordials } from "ext:core/mod.js";
import { validateInteger } from "ext:deno_node/internal/validators.mjs";
import _mod1 from "ext:deno_node/internal/errors.ts";
const { ERR_INVALID_ARG_VALUE } = _mod1.codes;
"use strict";

const {
  MathFloor,
  NumberIsInteger,
} = primordials;

// TODO (fix): For some reason Windows CI fails with bigger hwm.
let defaultHighWaterMarkBytes = process.platform === "win32"
  ? 16 * 1024
  : 64 * 1024;
let defaultHighWaterMarkObjectMode = 16;

function highWaterMarkFrom(options, isDuplex, duplexKey) {
  return options.highWaterMark != null
    ? options.highWaterMark
    : isDuplex
    ? options[duplexKey]
    : null;
}

function getDefaultHighWaterMark(objectMode) {
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

const _defaultExport2 = {
  getHighWaterMark,
  getDefaultHighWaterMark,
  setDefaultHighWaterMark,
};

export default _defaultExport2;
export { getDefaultHighWaterMark, getHighWaterMark, setDefaultHighWaterMark };
