// TODO: implement the 'NodeJS.Timeout' and 'NodeJS.Immediate' versions of the timers.
// https://github.com/DefinitelyTyped/DefinitelyTyped/blob/1163ead296d84e7a3c80d71e7c81ecbd1a130e9a/types/node/v12/globals.d.ts#L1120-L1131
export const setTimeout = window.setTimeout;
export const clearTimeout = window.clearTimeout;
export const setInterval = window.setInterval;
export const clearInterval = window.clearInterval;
export const setImmediate = (handler: TimerHandler, ...args): number =>
  window.setTimeout(handler, 0, ...args);
export const clearImmediate = window.clearTimeout;
