// Copyright 2018-2025 the Deno authors. MIT license.
import { Buffer } from "node:buffer";
import {
  clearImmediate,
  clearInterval,
  clearTimeout,
  setImmediate,
  setInterval,
  setTimeout,
} from "node:timers";
import { performance } from "node:perf_hooks";
globalThis.Buffer = Buffer;
globalThis.clearImmediate = clearImmediate;
globalThis.clearInterval = clearInterval;
globalThis.clearTimeout = clearTimeout;
globalThis.global = globalThis;
globalThis.performance = performance;
globalThis.setImmediate = setImmediate;
globalThis.setInterval = setInterval;
globalThis.setTimeout = setTimeout;
