// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-lint-ignore-file

function getDefaultHighWaterMark(objectMode) {
  return objectMode ? 16 : 16 * 1024;
}

export default { getDefaultHighWaterMark };
export { getDefaultHighWaterMark };
