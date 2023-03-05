// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { promisify } from "internal:deno_node/util.ts";
import timers from "internal:deno_node/timers.ts";

export const setTimeout = promisify(timers.setTimeout),
  setImmediate = promisify(timers.setImmediate),
  setInterval = promisify(timers.setInterval);

export default {
  setTimeout,
  setImmediate,
  setInterval,
};
