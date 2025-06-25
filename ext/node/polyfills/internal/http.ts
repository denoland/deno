// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { setUnrefTimeout } from "node:timers";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { primordials } from "ext:core/mod.js";
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
  setUnrefTimeout(resetCache, 1000 - DatePrototypeGetMilliseconds(d));
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
