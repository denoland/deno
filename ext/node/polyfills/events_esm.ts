// Copyright 2018-2026 the Deno authors. MIT license.
// @deno-types="./_events.d.ts"

import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/_events.mjs");

export const {
  addAbortListener,
  captureRejectionSymbol,
  defaultMaxListeners,
  errorMonitor,
  EventEmitter,
  EventEmitterAsyncResource,
  getEventListeners,
  getMaxListeners,
  listenerCount,
  on,
  once,
  setMaxListeners,
} = mod;

export default mod.default;
