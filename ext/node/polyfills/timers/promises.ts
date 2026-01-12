// Copyright 2018-2026 the Deno authors. MIT license.

import timers from "node:timers";

export const setTimeout = timers.promises.setTimeout;
export const setInterval = timers.promises.setInterval;
export const setImmediate = timers.promises.setImmediate;
export const scheduler = timers.promises.scheduler;

export default {
  setTimeout,
  setImmediate,
  setInterval,
  scheduler,
};
