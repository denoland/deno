// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
(function () {
const { core, primordials } = __bootstrap;
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
// Native fast-path mode: when set on a ServerResponse, `end()`/`write()` commit
// the response directly via the op_http_* ops (the deno_http_h1 engine) instead
// of serializing to a socket. Set by the node:http native dispatch.
const kNativeExternal = Symbol("kNativeExternal");
const kNativeWriteBuf = Symbol("kNativeWriteBuf");

const _defaultExport = {
  utcDate,
  kOutHeaders,
  kNeedDrain,
  kNativeExternal,
  kNativeWriteBuf,
};

return {
  utcDate,
  kOutHeaders,
  kNeedDrain,
  kNativeExternal,
  kNativeWriteBuf,
  default: _defaultExport,
};
})();
