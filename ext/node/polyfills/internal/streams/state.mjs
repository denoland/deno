// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-lint-ignore-file

// TODO(bartlomieju): this should be 64?
let defaultHighWaterMarkBytes = 16 * 1024;
let defaultHighWaterMarkObjectMode = 16;

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

export default { getDefaultHighWaterMark, setDefaultHighWaterMark };
export { getDefaultHighWaterMark, setDefaultHighWaterMark };
