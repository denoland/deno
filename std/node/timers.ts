// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// TODO(bartlomieju): implement the 'NodeJS.Timeout' and 'NodeJS.Immediate' versions of the timers.
// https://github.com/DefinitelyTyped/DefinitelyTyped/blob/1163ead296d84e7a3c80d71e7c81ecbd1a130e9a/types/node/v12/globals.d.ts#L1120-L1131
export const setTimeout = globalThis.setTimeout;
export const clearTimeout = globalThis.clearTimeout;
export const setInterval = globalThis.setInterval;
export const clearInterval = globalThis.clearInterval;
export const setImmediate = (
  // deno-lint-ignore no-explicit-any
  cb: (...args: any[]) => void,
  // deno-lint-ignore no-explicit-any
  ...args: any[]
): number => globalThis.setTimeout(cb, 0, ...args);
export const clearImmediate = globalThis.clearTimeout;

export default {
  setTimeout,
  clearTimeout,
  setInterval,
  clearInterval,
  setImmediate,
  clearImmediate,
};
