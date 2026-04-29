// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

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
  core.createSystemTimer(resetCache, 1000 - DatePrototypeGetMilliseconds(d));
}

function resetCache() {
  utcCache = undefined;
}

export const kOutHeaders = Symbol("kOutHeaders");
export const kNeedDrain = Symbol("kNeedDrain");

export default {
  utcDate,
  kOutHeaders,
  kNeedDrain,
};
