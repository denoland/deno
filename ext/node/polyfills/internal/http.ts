// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import { core, primordials } from "ext:core/mod.js";
const {
  Date,
  DatePrototypeToUTCString,
  DatePrototypeGetMilliseconds,
  Symbol,
} = primordials;

let utcCache: string | undefined;

export function utcDate() {
  if (!utcCache) cache();
  return utcCache;
}

function cache() {
  const d = new Date();
  utcCache = DatePrototypeToUTCString(d);
  // Use core.createTimer as a system timer so it doesn't participate
  // in Deno's test sanitizer checks.
  // args: callback, after, args, isRepeat, isRefed, isSystem
  core.createTimer(
    resetCache,
    1000 - DatePrototypeGetMilliseconds(d),
    undefined,
    false,
    false,
    true,
  );
}

function resetCache() {
  utcCache = undefined;
}

export function emitStatistics(
  _statistics: { startTime: [number, number] } | null,
) {
  notImplemented("internal/http.emitStatistics");
}

export const kOutHeaders = Symbol("kOutHeaders");
export const kNeedDrain = Symbol("kNeedDrain");

export default {
  utcDate,
  emitStatistics,
  kOutHeaders,
  kNeedDrain,
};
