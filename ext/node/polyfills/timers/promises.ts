// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { promisify } from "node:util";
import timers from "node:timers";

export const setTimeout = promisify(timers.setTimeout),
  setImmediate = promisify(timers.setImmediate),
  setInterval = promisify(timers.setInterval);

export default {
  setTimeout,
  setImmediate,
  setInterval,
};
