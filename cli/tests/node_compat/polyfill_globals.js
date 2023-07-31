// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import process from "node:process";
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
import console from "node:console";
globalThis.Buffer = Buffer;
globalThis.clearImmediate = clearImmediate;
globalThis.clearInterval = clearInterval;
globalThis.clearTimeout = clearTimeout;
globalThis.console = console;
globalThis.global = globalThis;
globalThis.performance = performance;
globalThis.process = process;
globalThis.setImmediate = setImmediate;
globalThis.setInterval = setInterval;
globalThis.setTimeout = setTimeout;
delete globalThis.window;
