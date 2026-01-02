// Copyright 2018-2026 the Deno authors. MIT license.
// @deno-types="./_events.d.ts"
export {
  addAbortListener,
  captureRejectionSymbol,
  default,
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
} from "ext:deno_node/_events.mjs";
