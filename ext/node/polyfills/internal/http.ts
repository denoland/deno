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
// The request's cancel watcher (op_http_request_on_cancel promise), armed when
// a handler returns without committing its response. An end() on such a
// response defers the 'finish'/'close' choice to this verdict: it resolves
// true when the client went away before the engine flushed the response
// (Node never emits 'finish' for those), false once the body is written.
const kNativeCancelWatch = Symbol("kNativeCancelWatch");

const _defaultExport = {
  utcDate,
  kOutHeaders,
  kNeedDrain,
  kNativeExternal,
  kNativeWriteBuf,
  kNativeCancelWatch,
};

return {
  utcDate,
  kOutHeaders,
  kNeedDrain,
  kNativeExternal,
  kNativeWriteBuf,
  kNativeCancelWatch,
  default: _defaultExport,
};
})();
