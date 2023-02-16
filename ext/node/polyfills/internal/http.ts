// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { setUnrefTimeout } from "internal:deno_node/polyfills/timers.ts";
import { notImplemented } from "internal:deno_node/polyfills/_utils.ts";

let utcCache: string | undefined;

export function utcDate() {
  if (!utcCache) cache();
  return utcCache;
}

function cache() {
  const d = new Date();
  utcCache = d.toUTCString();
  setUnrefTimeout(resetCache, 1000 - d.getMilliseconds());
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
