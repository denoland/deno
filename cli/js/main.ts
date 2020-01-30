// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { bootstrapMainRuntime } from "./runtime_main.ts";
import {
  bootstrapWorkerRuntime,
  runWorkerMessageLoop
} from "./runtime_worker.ts";

Object.defineProperties(globalThis, {
  bootstrapMainRuntime: {
    value: bootstrapMainRuntime,
    enumerable: false,
    writable: false,
    configurable: false
  },
  bootstrapWorkerRuntime: {
    value: bootstrapWorkerRuntime,
    enumerable: false,
    writable: false,
    configurable: false
  },
  runWorkerMessageLoop: {
    value: runWorkerMessageLoop,
    enumerable: false,
    writable: false,
    configurable: false
  }
});
