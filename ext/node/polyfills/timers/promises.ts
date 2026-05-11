// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = globalThis.__bootstrap;
const timers = core.loadExtScript("ext:deno_node/timers.ts").default;

const setTimeout = timers.promises.setTimeout;
const setImmediate = timers.promises.setImmediate;
const setInterval = timers.promises.setInterval;
const scheduler = timers.promises.scheduler;

return {
  default: {
    setTimeout,
    setImmediate,
    setInterval,
    scheduler,
  },
  setTimeout,
  setImmediate,
  setInterval,
  scheduler,
};
})();
