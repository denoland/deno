// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-fmt-ignore-file
(function () {
  const { core, primordials } = globalThis.__bootstrap;
  const {
    Date,
    DatePrototypeToUTCString,
    DatePrototypeGetMilliseconds,
    Symbol,
  } = primordials;

  let utcCache: string | undefined;

  function utcDate() {
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

  const kOutHeaders = Symbol("kOutHeaders");
  const kNeedDrain = Symbol("kNeedDrain");

  const __default_export__ = {
    utcDate,
    kOutHeaders,
    kNeedDrain,
  };

  return {
    utcDate,
    kOutHeaders,
    kNeedDrain,
    default: __default_export__,
  };
})()
