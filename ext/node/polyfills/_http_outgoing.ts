// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

import { core, internals, primordials } from "ext:core/mod.js";
import {
  op_http_abort_response,
  op_http_close_after_finish,
  op_http_get_request_cancelled,
  op_http_set_promise_complete,
  op_http_set_response_body_bytes,
  op_http_set_response_body_bytes_with_headers,
  op_http_set_response_body_resource,
  op_http_set_response_body_text,
  op_http_set_response_body_text_with_headers,
  op_http_set_response_close_delimited,
  op_http_set_response_force_close,
  op_http_set_response_headers,
  op_http_set_response_status_message,
  op_http_set_response_trailers,
} from "ext:core/ops";
const {
  Array,
  ArrayIsArray,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypePushApply,
  ArrayPrototypeShift,
  ArrayPrototypeUnshift,
  Error,
  FunctionPrototypeBind,
  FunctionPrototypeCall,
  MathFloor,
  NumberPrototypeToString,
  ObjectCreate,
  ObjectDefineProperties,
  ObjectDefineProperty,
  ObjectGetOwnPropertyDescriptors,
  ObjectKeys,
  ObjectSetPrototypeOf,
  ObjectValues,
  PromisePrototypeThen,
  SafeRegExp,
  SafeSet,
  String,
  StringPrototypeToLowerCase,
  StringPrototypeToString,
  Symbol,
  SymbolIterator,
  TypedArrayPrototypeGetByteLength,
} = primordials;
const { getDefaultHighWaterMark } = core.loadExtScript(
  "ext:deno_node/internal/streams/state.js",
);
const assert = core.loadExtScript(
  "ext:deno_node/internal/assert.mjs",
);
const { EventEmitter: EE } = core.loadExtScript("ext:deno_node/_events.mjs");
import { Stream } from "node:stream";
const { deprecate } = core.loadExtScript("ext:deno_node/util.ts");
import type { Socket } from "node:net";
const {
  kNativeExternal,
  kNativeWriteBuf,
  kNeedDrain,
  kOutHeaders,
  utcDate,
} = core.loadExtScript("ext:deno_node/internal/http.ts");
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
import {
  _checkInvalidHeaderChar as checkInvalidHeaderChar,
  _checkIsHttpToken as checkIsHttpToken,
  chunkExpression as RE_TE_CHUNKED,
} from "node:_http_common";
const {
  defaultTriggerAsyncIdScope,
  symbols,
} = core.loadExtScript("ext:deno_node/internal/async_hooks.ts");
const { async_id_symbol } = symbols;
const {
  ERR_HTTP_BODY_NOT_ALLOWED,
  ERR_HTTP_CONTENT_LENGTH_MISMATCH,
  ERR_HTTP_HEADERS_SENT,
  ERR_HTTP_INVALID_HEADER_VALUE,
  ERR_HTTP_TRAILER_INVALID,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_CHAR,
  ERR_INVALID_HTTP_TOKEN,
  ERR_METHOD_NOT_IMPLEMENTED,
  ERR_STREAM_CANNOT_PIPE,
  ERR_STREAM_DESTROYED,
  ERR_STREAM_NULL_VALUES,
  ERR_STREAM_WRITE_AFTER_END,
  errnoException,
  hideStackFrames,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  kLastWriteWasAsync,
  streamBaseState,
} = core.loadExtScript("ext:deno_node/internal_binding/stream_wrap.ts");
const { validateString } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const { isUint8Array } = core.loadExtScript(
  "ext:deno_node/internal/util/types.ts",
);

const { debuglog } = core.loadExtScript(
  "ext:deno_node/internal/util/debuglog.ts",
);
let debug = debuglog("http", (fn) => {
  debug = fn;
});

const HIGH_WATER_MARK = getDefaultHighWaterMark();

export const kUniqueHeaders = Symbol("kUniqueHeaders");
export const kHighWaterMark = Symbol("kHighWaterMark");
const kCorked = Symbol("corked");
const kSocket = Symbol("kSocket");
const kChunkedBuffer = Symbol("kChunkedBuffer");
const kChunkedLength = Symbol("kChunkedLength");
const kBytesWritten = Symbol("kBytesWritten");
export const kRejectNonStandardBodyWrites = Symbol(
  "kRejectNonStandardBodyWrites",
);

const nop = () => {};

const RE_CONN_CLOSE = new SafeRegExp(/(?:^|\W)close(?:$|\W)/i);

function isCookieField(s: string) {
  return s.length === 6 && StringPrototypeToLowerCase(s) === "cookie";
}

// ===========================================================================
// Native fast-path helpers. When a ServerResponse is in native mode
// (`this[kNativeExternal]` set), its writeHead/write/end commit the response
// straight to the deno_http_h1 engine via op_http_* ops, bypassing socket
// serialization. See ext/node/polyfills/_http_server.js (native dispatch).
// ===========================================================================

// Inert truthy value for `_header` so `headersSent` reads true and a second
// writeHead throws -- native mode never writes `_header` to a socket.
const NATIVE_HEADER_SENT = "\r\n";

// Holds the ReadableStream controller once a native response switches to
// incremental streaming (see nativeStartStream). Absent => not streaming.
const kNativeStream = Symbol("kNativeStream");
// Structured trailer list ([[name, value], ...]) collected by addTrailers in
// native mode, committed via op_http_set_response_trailers before the response
// body. Setting trailers on the record makes the engine frame the response as
// chunked and emit them after the body.
const kNativeTrailers = Symbol("kNativeTrailers");

// Build the wire header list ([[name, value], ...]) from kOutHeaders. Multi-
// value headers expand to repeated entries. Returns null if there are none.
function nativeWireHeaders(msg: any) {
  const oh = msg[kOutHeaders];
  const out: [string, string][] = [];
  let hasConnection = false;
  let hasKeepAlive = false;
  let hasDate = false;
  if (oh !== null && oh !== undefined) {
    // deno-lint-ignore guard-for-in
    for (const key in oh) {
      const entry = oh[key];
      const name = entry[0];
      const value = entry[1];
      if (key === "connection") {
        hasConnection = true;
      } else if (key === "keep-alive") {
        hasKeepAlive = true;
      } else if (key === "date") {
        hasDate = true;
      }
      if (ArrayIsArray(value)) {
        for (let i = 0; i < value.length; i++) {
          ArrayPrototypePush(out, [name, String(value[i])]);
        }
      } else {
        ArrayPrototypePush(out, [name, String(value)]);
      }
    }
  }
  // A write()'d response with no explicit framing is sent chunked (see
  // nativeEnd); the engine frames it chunked when it sees this header.
  if (msg._nativeChunked) {
    ArrayPrototypePush(out, ["Transfer-Encoding", "chunked"]);
  }
  // Node adds the Date header itself (honoring res.sendDate / removeHeader);
  // the engine doesn't auto-add it for node responses (raw_response_headers).
  if (!hasDate && msg.sendDate) {
    ArrayPrototypePush(out, ["Date", utcDate()]);
  }
  // Node emits an explicit `Connection: keep-alive` (+ `Keep-Alive`) on HTTP/1.1
  // keep-alive responses; the engine omits it (1.1 default) and only writes it
  // for HTTP/1.0, so add it here for 1.1 only -- unless the handler removed or
  // set its own connection header, or this is the last response on the socket.
  const req = msg.req;
  if (
    !hasConnection && !msg._removedConnection && !msg._last &&
    msg.shouldKeepAlive && req !== undefined && req.httpVersionMinor === 1
  ) {
    ArrayPrototypePush(out, ["Connection", "keep-alive"]);
    const kat = msg._keepAliveTimeout;
    if (!hasKeepAlive && typeof kat === "number" && kat > 0) {
      ArrayPrototypePush(out, [
        "Keep-Alive",
        "timeout=" + MathFloor(kat / 1000),
      ]);
    }
  }
  return out.length === 0 ? null : out;
}

function nativeChunkToBuffer(chunk: any, encoding?: string | null) {
  if (typeof chunk === "string") {
    return Buffer.from(chunk, encoding || "utf8");
  }
  return chunk;
}

// _contentLength is normally set by _matchHeader during _storeHeader, which the
// native path skips, so derive it from the Content-Length header in kOutHeaders.
// Returns whether strictContentLength should be enforced for this response.
function nativeContentLength(msg: any): boolean {
  if (msg._contentLength == null && msg[kOutHeaders]) {
    const cl = msg[kOutHeaders]["content-length"];
    if (cl !== undefined) {
      msg._contentLength = +cl[1];
    }
  }
  return msg.strictContentLength && _checkStrictContentLength(msg);
}

// The native write/end fast paths bypass write_(), so replicate its byte
// accounting: enforce strictContentLength (throws ERR_HTTP_CONTENT_LENGTH_MISMATCH
// on over/under-run) and accumulate the body bytes into `kBytesWritten` and the
// synthetic socket's `bytesWritten` (which the engine, not the socket, actually
// writes through). `fromEnd` requires an exact match; mid-stream requires
// not-exceeding.
function nativeAccountChunk(
  msg: any,
  chunk: any,
  encoding: string | null,
  fromEnd: boolean,
) {
  if (chunk === undefined || chunk === null || chunk === "") {
    return;
  }
  const len = typeof chunk === "string"
    ? Buffer.byteLength(chunk, encoding || "utf8")
    : TypedArrayPrototypeGetByteLength(chunk);
  if (
    nativeContentLength(msg) &&
    (fromEnd
      ? msg[kBytesWritten] + len !== msg._contentLength
      : msg[kBytesWritten] + len > msg._contentLength)
  ) {
    throw new ERR_HTTP_CONTENT_LENGTH_MISMATCH(
      len + msg[kBytesWritten],
      msg._contentLength,
    );
  }
  msg[kBytesWritten] += len;
  const sock = msg.socket;
  if (
    sock !== undefined && sock !== null && typeof sock.bytesWritten === "number"
  ) {
    sock.bytesWritten += len;
  }
}

// Commit the response in a single op. `body` is a string (UTF-8 fast path),
// a Buffer/Uint8Array, or null/undefined for no body.
function nativeCommit(msg: any, body: any) {
  const external = msg[kNativeExternal];
  msg[kNativeExternal] = null;
  // The response now owns the external (the body-commit op below consumes it);
  // mark the request so a later `req.destroy()` doesn't abort via the now-stale
  // external (would be a use-after-free).
  if (msg.req !== undefined && msg.req !== null) {
    msg.req._nativeResponded = true;
  }
  // writeHead() locks the status (a later `res.statusCode = ...` is a no-op in
  // Node); fall back to the live statusCode when end() committed without one.
  const status = msg._nativeWriteHead ? msg._nativeStatus : msg.statusCode;
  // RFC 2616: a 204/304 MUST NOT have a body, so a user-set Transfer-Encoding:
  // chunked is unframeable -- Node suppresses the chunk and forces the
  // connection closed (no keep-alive). Tell the engine to close after this
  // response (borrows the external; before the body-commit op consumes it).
  if (msg._nativeForceClose) {
    // Something (e.g. a handler emitting 'close' on the socket to abort
    // pipelining) requested the connection be closed after this response.
    op_http_set_response_force_close(external);
  }
  if (
    (status === 204 || status === 304) && msg.hasHeader("transfer-encoding")
  ) {
    msg.shouldKeepAlive = false;
    op_http_set_response_force_close(external);
  } else if (msg._hasBody && msg._removedContLen && msg._removedTE) {
    // The handler removed both Content-Length and Transfer-Encoding, so the body
    // has no length framing: send it close-delimited (no content-length header;
    // the connection closes to mark the end). Matches Node's _storeHeader, which
    // sets `_last = true` when both are removed.
    msg.shouldKeepAlive = false;
    op_http_set_response_close_delimited(external);
  }
  // No writeHead() but a custom `res.statusMessage` was set (writeHead handles
  // its own case and sets `_nativeWriteHead`): push the reason phrase. Truthy
  // statusMessage without writeHead is always user-supplied, so the hot path
  // (no statusMessage) skips this.
  if (msg.statusMessage && !msg._nativeWriteHead) {
    op_http_set_response_status_message(external, msg.statusMessage);
  }
  // Set trailers on the record first (borrows the external); the engine then
  // frames the response as chunked and appends them after the body. Must run
  // before the body-commit op below, which consumes the external.
  const trailers = msg[kNativeTrailers];
  if (trailers !== undefined && trailers !== null && trailers.length > 0) {
    op_http_set_response_trailers(external, trailers);
  }
  const headers = nativeWireHeaders(msg);
  if (
    body === undefined || body === null || body === "" ||
    msg._hasBody === false
  ) {
    if (headers !== null) {
      op_http_set_response_headers(external, headers);
    }
    op_http_set_promise_complete(external, status);
  } else if (typeof body === "string") {
    if (headers !== null) {
      op_http_set_response_body_text_with_headers(
        external,
        body,
        status,
        headers,
      );
    } else {
      op_http_set_response_body_text(external, body, status);
    }
  } else {
    if (headers !== null) {
      op_http_set_response_body_bytes_with_headers(
        external,
        body,
        status,
        headers,
      );
    } else {
      op_http_set_response_body_bytes(external, body, status);
    }
  }
}

function nativeEnd(msg: any, chunk: any, encoding: any, callback: any) {
  if (typeof chunk === "function") {
    callback = chunk;
    chunk = null;
    encoding = null;
  } else if (typeof encoding === "function") {
    callback = encoding;
    encoding = null;
  }

  // The native path bypasses write_(); replicate its chunk-type validation
  // (null/undefined are fine for end() = no body).
  if (
    chunk !== undefined && chunk !== null && typeof chunk !== "string" &&
    !isUint8Array(chunk)
  ) {
    throw new ERR_INVALID_ARG_TYPE(
      "chunk",
      ["string", "Buffer", "Uint8Array"],
      chunk,
    );
  }
  // strictContentLength + byte accounting for the final chunk, then a final
  // exact-match check that also catches an under-run (writes summed below the
  // declared Content-Length with no make-up chunk at end()).
  nativeAccountChunk(msg, chunk, encoding, true);
  if (nativeContentLength(msg) && msg[kBytesWritten] !== msg._contentLength) {
    throw new ERR_HTTP_CONTENT_LENGTH_MISMATCH(
      msg[kBytesWritten],
      msg._contentLength,
    );
  }

  let body;
  const buffered = msg[kNativeWriteBuf];
  if (buffered !== undefined && buffered !== null) {
    if (chunk !== undefined && chunk !== null && chunk !== "") {
      ArrayPrototypePush(buffered, nativeChunkToBuffer(chunk, encoding));
    }
    msg[kNativeWriteBuf] = null;
    // The buffered writes are now committed into the flat body.
    msg[kChunkedLength] = 0;
    if (buffered.length === 0) {
      body = null;
    } else if (buffered.length === 1) {
      body = buffered[0];
    } else {
      // deno-lint-ignore prefer-primordials -- Buffer.concat is a static Buffer method, not Array/TypedArray concat
      body = Buffer.concat(buffered);
    }
  } else if (
    typeof chunk === "string" && encoding && encoding !== "utf8" &&
    encoding !== "utf-8"
  ) {
    body = Buffer.from(chunk, encoding);
  } else {
    body = chunk; // string (utf8 fast path), Buffer, or null
  }

  if (!msg._header) {
    msg._header = NATIVE_HEADER_SENT;
  }
  // Node chunks a response that used write() (it can't know the total length at
  // write() time) when no explicit Content-Length/Transfer-Encoding was set; our
  // flat coalescing path would otherwise send Content-Length. Opt into chunked
  // framing to match. (write() created kNativeWriteBuf; end()-only did not.)
  if (
    buffered !== undefined && buffered !== null && msg._hasBody !== false &&
    msg.req !== undefined && msg.req.httpVersionMinor === 1 &&
    !msg.hasHeader("content-length") && !msg.hasHeader("transfer-encoding") &&
    !msg._removedTE
  ) {
    msg._nativeChunked = true;
  }
  // Was the client already gone? Check before nativeCommit consumes the
  // external; an aborted response emits 'close' (not 'finish').
  const ext = msg[kNativeExternal];
  const aborted = ext ? op_http_get_request_cancelled(ext) : false;
  // `finished` is settable; `writableEnded`/`writableFinished` are getters
  // derived from it (+ outputSize/socket, both empty here), so don't assign them.
  msg.finished = true;
  // Response complete without the handler consuming the request body: discard it
  // (Node dumps the unread body on finish). Done synchronously here so a body
  // read scheduled by an attached 'data' listener can't deliver the body or
  // touch the just-consumed external.
  const reqEnd = msg.req;
  if (reqEnd !== undefined && reqEnd !== null && reqEnd._nativeDiscardBody) {
    reqEnd._nativeDiscardBody();
  }
  nativeCommit(msg, body);
  nativeEmitFinish(msg, callback, aborted);
  return msg;
}

// Emit prefinish/finish asynchronously (once) for a committed native response.
function nativeEmitFinish(msg: any, callback: any, aborted?: boolean) {
  const finish = () => {
    if (msg._nativeFinishEmitted) return;
    msg._nativeFinishEmitted = true;
    if (aborted) {
      // The client went away before the response completed: Node emits 'close'
      // (the response never 'finish'es). Emitting only one keeps tests that
      // register the same listener on both events correct
      // (test-http-writable-true-after-close).
      if (!msg._closed && !msg.destroyed) {
        msg._closed = true;
        msg.destroyed = true;
        msg.emit("close");
      }
      return;
    }
    // If the connection closes after this response (HTTP/1.0 or Connection:
    // close), end the synthetic socket's writable side first so
    // `socket.writableEnded` reflects it before 'finish' listeners run.
    const sock = msg.socket;
    if (
      !msg.shouldKeepAlive && sock !== undefined && sock !== null &&
      !sock.writableEnded && typeof sock.end === "function"
    ) {
      sock.end();
    }
    msg.emit("prefinish");
    msg.emit("finish");
    // Mark the response closed/destroyed one extra tick later (Node sets
    // res.destroyed at 'close'; `res.writable` stays true as it's a plain
    // field). The extra tick makes a *deferred* (setImmediate) write-after-end
    // route ERR_STREAM_WRITE_AFTER_END to its callback with no 'error' event
    // (write_ takes the destroyed branch) -- while a *synchronous*
    // write-after-end, whose error tick runs before this, still sees the
    // response open and emits 'error'.
    (globalThis as any).process.nextTick(() => {
      if (!msg._closed && !msg.destroyed) {
        msg._closed = true;
        msg.destroyed = true;
        msg.emit("close");
      }
      // Drain the request so it 'end's (then autoDestroy -> 'close') after the
      // response closes, matching Node's ordering. Only when the body is fully
      // received (`complete`, so we never read a freed external) AND something
      // is listening for the lifecycle -- the hot path (no listener) skips this
      // to avoid per-request drain/destroy overhead.
      const req = msg.req;
      if (
        req !== undefined && req !== null && req.complete &&
        !req.readableEnded && !req.destroyed &&
        (req.listenerCount("end") > 0 || req.listenerCount("close") > 0 ||
          req.listenerCount("data") > 0)
      ) {
        req.resume();
      }
    });
  };
  (globalThis as any).process.nextTick(finish);
  if (typeof callback === "function") {
    msg.once("finish", callback);
  }
}

// Switch a still-open native response to incremental streaming: commit the
// headers/status now and back the body with a ReadableStream-derived resource
// that the deno_http_h1 engine drains as chunks are enqueued. Used when a
// handler calls write() and yields without ending in the same tick (e.g. SSE,
// chunked responses). Buffered write() chunks are flushed into the stream.
function nativeStartStream(msg: any) {
  const external = msg[kNativeExternal];
  // Note whether the client already went away before we consume the external;
  // nativeEndStream uses it to emit 'close' instead of 'finish'.
  msg._nativeAborted = op_http_get_request_cancelled(external);
  // The streaming resource borrows the external; we own its lifecycle now and
  // free it via op_http_close_after_finish once the body fully drains.
  msg[kNativeExternal] = null;
  // The streaming response now owns the external; see nativeCommit.
  if (msg.req !== undefined && msg.req !== null) {
    msg.req._nativeResponded = true;
  }
  // writeHead() locks the status (see nativeCommit).
  const status = msg._nativeWriteHead ? msg._nativeStatus : msg.statusCode;
  const headers = nativeWireHeaders(msg);
  if (headers !== null) {
    op_http_set_response_headers(external, headers);
  }
  let controller: any;
  const stream = new (globalThis as any).ReadableStream({
    start(c: any) {
      controller = c;
    },
    cancel() {
      // Client went away; nothing more to push.
    },
  });
  msg[kNativeStream] = controller;
  if (!msg._header) {
    msg._header = NATIVE_HEADER_SENT;
  }
  const buffered = msg[kNativeWriteBuf];
  msg[kNativeWriteBuf] = null;
  if (buffered !== undefined && buffered !== null) {
    for (let i = 0; i < buffered.length; i++) {
      controller.enqueue(buffered[i]);
    }
  }
  // The buffered chunks were flushed to the stream: clear the accounted length
  // and release any writer that backpressured.
  msg[kChunkedLength] = 0;
  if (msg[kNeedDrain]) {
    msg[kNeedDrain] = false;
    (globalThis as any).process.nextTick(() => msg.emit("drain"));
  }
  // noAggregate: preserve per-write buffer boundaries so each res.write becomes
  // its own HTTP chunk on the wire, matching Node's chunked framing.
  const rid = internals.resourceForReadableStream(
    stream,
    undefined,
    undefined,
    true,
  );
  PromisePrototypeThen(
    op_http_set_response_body_resource(external, rid, true, status),
    () => op_http_close_after_finish(external),
    () => op_http_close_after_finish(external),
  );
}

// Deferred check armed on the first buffered write(): if the response is still
// open (not ended via the single-op fast path) and hasn't already streamed,
// promote it to a streaming body so the buffered chunks are flushed to the
// client instead of waiting for end().
function nativeMaybeStream(msg: any) {
  if (msg[kNativeExternal] === null || msg[kNativeExternal] === undefined) {
    return; // already committed (single-op end) or already streaming
  }
  if (msg.finished) {
    return;
  }
  nativeStartStream(msg);
}

// end() for a response that is already streaming: enqueue the final chunk and
// close the stream so the engine finishes the body.
function nativeEndStream(msg: any, chunk: any, encoding: any, callback: any) {
  if (typeof chunk === "function") {
    callback = chunk;
    chunk = null;
    encoding = null;
  } else if (typeof encoding === "function") {
    callback = encoding;
    encoding = null;
  }
  const controller = msg[kNativeStream];
  msg[kNativeStream] = null;
  if (chunk !== undefined && chunk !== null && chunk !== "") {
    try {
      controller.enqueue(nativeChunkToBuffer(chunk, encoding));
    } catch { /* stream already errored/closed */ }
  }
  try {
    controller.close();
  } catch { /* already closed */ }
  msg.finished = true;
  // Discard an unread request body on finish (Node dumps it), same as nativeEnd.
  const reqEnd = msg.req;
  if (reqEnd !== undefined && reqEnd !== null && reqEnd._nativeDiscardBody) {
    reqEnd._nativeDiscardBody();
  }
  nativeEmitFinish(msg, callback, msg._nativeAborted);
  return msg;
}

export function OutgoingMessage(options?: any) {
  FunctionPrototypeCall(Stream, this);

  this[kHighWaterMark] = options?.highWaterMark ?? getDefaultHighWaterMark();
  this[kRejectNonStandardBodyWrites] = options?.rejectNonStandardBodyWrites ??
    false;

  // Queue that holds all currently pending data, until the response will be
  // assigned to the socket (until it will its turn in the HTTP pipeline).
  this.outputData = [];

  // `outputSize` is an approximate measure of how much data is queued on this
  // response. `_onPendingData` will be invoked to update similar global
  // per-connection counter. That counter will be used to pause/unpause the
  // TCP socket and HTTP Parser and thus handle the backpressure.
  this.outputSize = 0;

  this.writable = true;
  this.destroyed = false;

  this._last = false;
  this.chunkedEncoding = false;
  this.shouldKeepAlive = true;
  this.maxRequestsOnConnectionReached = false;
  this._defaultKeepAlive = true;
  this.useChunkedEncodingByDefault = true;
  this.sendDate = false;
  this._removedConnection = false;
  this._removedContLen = false;
  this._removedTE = false;

  this._contentLength = null;
  this._hasBody = true;
  this._trailer = "";
  this[kNeedDrain] = false;

  this.finished = false;
  this._headerSent = false;
  this[kCorked] = 0;
  this._closed = false;

  this[kSocket] = null;
  this._header = null;
  this[kOutHeaders] = null;

  this._keepAliveTimeout = 0;

  this._onPendingData = nop;

  this.strictContentLength = false;
  this[kBytesWritten] = 0;
  this[kChunkedBuffer] = [];
  this[kChunkedLength] = 0;

  this._bodyWriter = null;
}

ObjectSetPrototypeOf(OutgoingMessage.prototype, Stream.prototype);
ObjectSetPrototypeOf(OutgoingMessage, Stream);

ObjectDefineProperties(
  OutgoingMessage.prototype,
  ObjectGetOwnPropertyDescriptors({
    get writableFinished() {
      return (
        this.finished &&
        this.outputSize === 0 &&
        (!this[kSocket] || this[kSocket].writableLength === 0)
      );
    },

    get writableObjectMode() {
      return false;
    },

    get writableLength() {
      return this.outputSize + this[kChunkedLength] +
        (this[kSocket] ? this[kSocket].writableLength : 0);
    },

    get writableHighWaterMark() {
      return this[kSocket]
        ? this[kSocket].writableHighWaterMark
        : this[kHighWaterMark] || HIGH_WATER_MARK;
    },

    get writableCorked() {
      const corked = this[kSocket] ? this[kSocket].writableCorked : 0;
      return corked + this[kCorked];
    },

    get connection() {
      return this[kSocket];
    },

    set connection(val) {
      this.socket = val;
    },

    get writableEnded() {
      return this.finished;
    },

    get writableNeedDrain() {
      return !this.destroyed && !this.finished && this[kNeedDrain];
    },

    cork() {
      if (this.socket) {
        this.socket.cork();
      } else {
        this[kCorked]++;
      }
    },

    uncork() {
      if (this.socket) {
        this.socket.uncork();
      } else if (this[kCorked]) {
        this[kCorked]--;
      }
    },

    setTimeout(msecs: number, callback?: (...args: unknown[]) => void) {
      if (callback) {
        this.on("timeout", callback);
      }

      if (!this.socket) {
        this.once("socket", function socketSetTimeoutOnConnect(socket: any) {
          socket.setTimeout(msecs);
        });
      } else {
        this.socket.setTimeout(msecs);
      }
      return this;
    },

    // It's possible that the socket will be destroyed, and removed from
    // any messages, before ever calling this.  In that case, just skip
    // it, since something else is destroying this connection anyway.
    destroy(error: unknown) {
      if (this.destroyed) {
        return this;
      }
      this.destroyed = true;

      // Native mode: abort an in-flight streaming response by erroring the
      // backing ReadableStream, so the engine tears the connection down and the
      // peer sees an ECONNRESET (matches `res.destroy()` on the classic path).
      const controller = this[kNativeStream];
      if (controller !== undefined && controller !== null) {
        this[kNativeStream] = null;
        try {
          controller.error(error || new Error("aborted"));
        } catch { /* already closed/errored */ }
      } else {
        // Native mode, response not yet committed: drop the connection so the
        // peer sees an ECONNRESET (the external is still live -- a commit op
        // would have nulled it).
        const ext = this[kNativeExternal];
        if (ext !== undefined && ext !== null) {
          this[kNativeExternal] = null;
          if (this.req !== undefined && this.req !== null) {
            this.req._nativeResponded = true;
          }
          op_http_abort_response(ext);
        }
      }

      if (this.socket) {
        this.socket.destroy(error);
      } else {
        this.once("socket", function socketDestroyOnConnect(socket: any) {
          socket.destroy(error);
        });
      }

      // This destroy() fully overrides Writable's, so emit the lifecycle events
      // Node would (async): 'error' (if any, and only when observed) then
      // 'close'. nativeEmitFinish's close path is guarded on the same `_closed`
      // flag, so the response emits 'close' exactly once.
      (globalThis as any).process.nextTick(() => {
        if (this._closed) {
          return;
        }
        this._closed = true;
        if (error && this.listenerCount("error") > 0) {
          this.emit("error", error);
        }
        this.emit("close");
      });

      return this;
    },

    setHeader(name: string, value: string) {
      if (this._header) {
        throw new ERR_HTTP_HEADERS_SENT("set");
      }
      validateHeaderName(name);
      validateHeaderValue(name, value);

      let headers = this[kOutHeaders];
      if (headers === null) {
        this[kOutHeaders] = headers = ObjectCreate(null);
      }

      name = StringPrototypeToString(name);
      headers[StringPrototypeToLowerCase(name)] = [name, value];
      return this;
    },

    appendHeader(name, value) {
      if (this._header) {
        throw new ERR_HTTP_HEADERS_SENT("append");
      }
      validateHeaderName(name);
      validateHeaderValue(name, value);

      name = StringPrototypeToString(name);

      const field = StringPrototypeToLowerCase(name);
      const headers = this[kOutHeaders];
      if (headers === null || !headers[field]) {
        return this.setHeader(name, value);
      }

      // Prepare the field for appending, if required
      if (!ArrayIsArray(headers[field][1])) {
        headers[field][1] = [headers[field][1]];
      }

      const existingValues = headers[field][1];
      if (ArrayIsArray(value)) {
        for (let i = 0, length = value.length; i < length; i++) {
          ArrayPrototypePush(existingValues, StringPrototypeToString(value[i]));
        }
      } else {
        ArrayPrototypePush(existingValues, StringPrototypeToString(value));
      }

      return this;
    },

    // Returns a shallow copy of the current outgoing headers.
    getHeaders() {
      const headers = this[kOutHeaders];
      const ret = ObjectCreate(null);
      if (headers) {
        const keys = ObjectKeys(headers);
        // Retain for(;;) loop for performance reasons
        // Refs: https://github.com/nodejs/node/pull/30958
        for (let i = 0; i < keys.length; ++i) {
          const key = keys[i];
          const val = headers[key][1];
          ret[key] = val;
        }
      }
      return ret;
    },

    hasHeader(name: string) {
      validateString(name, "name");
      return this[kOutHeaders] !== null &&
        !!this[kOutHeaders][StringPrototypeToLowerCase(name)];
    },

    removeHeader(name: string) {
      validateString(name, "name");

      if (this._header) {
        throw new ERR_HTTP_HEADERS_SENT("remove");
      }

      const key = StringPrototypeToLowerCase(name);

      switch (key) {
        case "connection":
          this._removedConnection = true;
          break;
        case "content-length":
          this._removedContLen = true;
          break;
        case "transfer-encoding":
          this._removedTE = true;
          break;
        case "date":
          this.sendDate = false;
          break;
      }

      if (this[kOutHeaders] !== null) {
        delete this[kOutHeaders][key];
      }
    },

    getHeader(name: string) {
      validateString(name, "name");

      const headers = this[kOutHeaders];
      if (headers === null) {
        return;
      }

      const entry = headers[StringPrototypeToLowerCase(name)];
      return entry && entry[1];
    },

    // Returns an array of the names of the current outgoing headers.
    getHeaderNames() {
      return this[kOutHeaders] !== null ? ObjectKeys(this[kOutHeaders]) : [];
    },

    // Returns an array of the names of the current outgoing raw headers.
    getRawHeaderNames() {
      const headersMap = this[kOutHeaders];
      if (headersMap === null) return [];

      const values = ObjectValues(headersMap);
      const headers = new Array(values.length);
      // Retain for(;;) loop for performance reasons
      // Refs: https://github.com/nodejs/node/pull/30958
      for (let i = 0, l = values.length; i < l; i++) {
        headers[i] = (values as any)[i][0];
      }

      return headers;
    },

    // Match Node: call the standalone write_() helper with `this` as the
    // first argument instead of a method call on `this`. This keeps the
    // chunk-type validation reachable when callers pass a fake `this` via
    // `outgoingMessage.write.call(fake)` (see lib/_http_outgoing.js).
    write(
      chunk: string | Uint8Array | Buffer,
      encoding: string | null,
      callback: () => void,
    ): boolean {
      if (typeof encoding === "function") {
        callback = encoding;
        encoding = null;
      }
      if (this[kNativeStream] || this[kNativeExternal]) {
        // The native fast paths below bypass write_(); replicate its chunk
        // validation so bad types throw the same errors Node does.
        if (chunk === null) {
          throw new ERR_STREAM_NULL_VALUES();
        }
        if (typeof chunk !== "string" && !isUint8Array(chunk)) {
          throw new ERR_INVALID_ARG_TYPE(
            "chunk",
            ["string", "Buffer", "Uint8Array"],
            chunk,
          );
        }
        nativeAccountChunk(this, chunk, encoding, false);
      }
      if (this[kNativeStream]) {
        // Already streaming: hand the chunk straight to the body stream. The
        // engine drains it as the socket allows. (Byte-aware backpressure with a
        // 'drain' signal is a follow-up; we return true as the buffered path did,
        // which never stalls a pipe().)
        if (chunk !== undefined && chunk !== null && chunk.length !== 0) {
          this[kNativeStream].enqueue(nativeChunkToBuffer(chunk, encoding));
        }
        if (typeof callback === "function") {
          (globalThis as any).process.nextTick(callback);
        }
        return true;
      }
      if (this[kNativeExternal]) {
        // Native mode: buffer chunks. On the first write, arm a deferred check
        // that promotes the response to a streaming body if the handler yields
        // without ending in this tick (e.g. SSE/chunked). A synchronous
        // write()+end() never trips it and stays a single-op commit (nativeEnd
        // coalesces the buffer).
        let buf = this[kNativeWriteBuf];
        if (buf === undefined || buf === null) {
          buf = this[kNativeWriteBuf] = [];
          (globalThis as any).process.nextTick(nativeMaybeStream, this);
        }
        if (chunk !== undefined && chunk !== null && chunk.length !== 0) {
          const b = nativeChunkToBuffer(chunk, encoding);
          ArrayPrototypePush(buf, b);
          // Track the buffered length so `writableLength` reflects it and
          // write() applies backpressure once the high-water mark is hit. Node
          // frames a chunked body on the wire, so account the framed length
          // (`<hex-len>\r\n<chunk>\r\n`).
          this[kChunkedLength] += nativeContentLength(this)
            ? b.length
            : NumberPrototypeToString(b.length, 16).length + 4 + b.length;
        }
        if (typeof callback === "function") {
          (globalThis as any).process.nextTick(callback);
        }
        if (this[kChunkedLength] >= this.writableHighWaterMark) {
          this[kNeedDrain] = true;
          return false;
        }
        return true;
      }
      const ret = write_(this, chunk, encoding, callback, false);
      if (!ret) {
        this[kNeedDrain] = true;
      }
      return ret;
    },

    addTrailers(headers: any) {
      this._trailer = "";
      const keys = ObjectKeys(headers);
      const isArray = ArrayIsArray(headers);
      // Native mode commits trailers as a structured list via
      // op_http_set_response_trailers (see nativeCommit); the serialized
      // `_trailer` string is only used by the classic socket path.
      const native = !!this[kNativeExternal];
      const nativeList = native ? [] : null;
      let field, value;
      for (let i = 0, l = keys.length; i < l; i++) {
        if (isArray) {
          field = headers[keys[i]][0];
          value = headers[keys[i]][1];
        } else {
          field = keys[i];
          value = headers[field];
        }
        if (typeof field !== "string" || !field || !checkIsHttpToken(field)) {
          throw new ERR_INVALID_HTTP_TOKEN("Trailer name", field);
        }
        if (checkInvalidHeaderChar(value)) {
          debug('Trailer "%s" contains invalid characters', field);
          throw new ERR_INVALID_CHAR("trailer content", field);
        }
        this._trailer += field + ": " + value + "\r\n";
        if (nativeList !== null) {
          ArrayPrototypePush(nativeList, [field, String(value)]);
        }
      }
      if (nativeList !== null) {
        this[kNativeTrailers] = nativeList;
      }
    },

    end(chunk: any, encoding: any, callback: any) {
      if (this[kNativeStream]) {
        return nativeEndStream(this, chunk, encoding, callback);
      }
      if (this[kNativeExternal]) {
        return nativeEnd(this, chunk, encoding, callback);
      }
      if (typeof chunk === "function") {
        callback = chunk;
        chunk = null;
        encoding = null;
      } else if (typeof encoding === "function") {
        callback = encoding;
        encoding = null;
      }

      if (chunk) {
        if (this.finished) {
          _onError(
            this,
            new ERR_STREAM_WRITE_AFTER_END(),
            typeof callback !== "function" ? nop : callback,
          );
          return this;
        }

        if (tryDirectEnd(this, chunk, encoding, callback)) {
          return this;
        }

        if (this.socket) {
          this.socket.cork();
        }

        write_(this, chunk, encoding, null, true);
      } else if (this.finished) {
        if (typeof callback === "function") {
          if (!this.writableFinished) {
            this.on("finish", callback);
          } else {
            callback(new Error("end already called"));
          }
        }
        return this;
      } else if (tryDirectEmptyEnd(this, callback)) {
        return this;
      } else if (!this._header) {
        if (this.socket) {
          this.socket.cork();
        }

        this._contentLength = 0;
        this._implicitHeader();
      }

      if (typeof callback === "function") {
        this.once("finish", callback);
      }

      if (
        _checkStrictContentLength(this) &&
        this[kBytesWritten] !== this._contentLength
      ) {
        throw new ERR_HTTP_CONTENT_LENGTH_MISMATCH(
          this[kBytesWritten],
          this._contentLength,
        );
      }

      const finish = FunctionPrototypeBind(onFinish, undefined, this);

      if (this._hasBody && this.chunkedEncoding) {
        this._send("0\r\n" + this._trailer + "\r\n", "latin1", finish);
      } else if (!this._headerSent || this.writableLength || chunk) {
        this._send("", "latin1", finish);
      } else {
        (globalThis as any).process.nextTick(finish);
      }

      if (this.socket) {
        // Fully uncork connection on end().
        this.socket._writableState.corked = 1;
        this.socket.uncork();
      } else {
        // No socket yet: fully uncork the buffered output. Only in the no-socket
        // case -- Node's uncork() decrements kCorked even with a socket, but
        // Deno's only does so without one (see cork()/uncork() below), so running
        // this with a socket would leave kCorked stuck at 1 and make
        // res.writableCorked disagree with res.socket.writableCorked
        // (test-http-response-cork).
        this[kCorked] = 1;
        this.uncork();
      }

      this.finished = true;

      // There is the first message on the outgoing queue, and we've sent
      // everything to the socket.
      if (
        this.outputData.length === 0 &&
        this.socket &&
        this.socket._httpMessage === this
      ) {
        this._finish();
      }

      return this;
    },

    flushHeaders() {
      // Native fast path: commit the headers immediately as a streaming
      // response (flushHeaders sends headers without ending the body, e.g.
      // full-duplex / SSE). The generic _send("") path doesn't go through the
      // native write buffer, so it would never reach the engine.
      if (this[kNativeStream]) {
        return; // already streaming (flushHeaders is idempotent)
      }
      if (this[kNativeExternal]) {
        nativeStartStream(this);
        return;
      }

      if (!this._header) {
        this._implicitHeader();
      }

      // Force-flush the headers.
      this._send("");
    },

    pipe() {
      // OutgoingMessage should be write-only. Piping from it is disabled.
      this.emit("error", new ERR_STREAM_CANNOT_PIPE());
    },

    _implicitHeader() {
      throw new ERR_METHOD_NOT_IMPLEMENTED("_implicitHeader()");
    },

    _finish() {
      assert(this.socket);
      this.emit("prefinish");
    },

    // This logic is probably a bit confusing. Let me explain a bit:
    //
    // In both HTTP servers and clients it is possible to queue up several
    // outgoing messages. This is easiest to imagine in the case of a client.
    // Take the following situation:
    //
    //    req1 = client.request('GET', '/');
    //    req2 = client.request('POST', '/');
    //
    // When the user does
    //
    //   req2.write('hello world\n');
    //
    // it's possible that the first request has not been completely flushed to
    // the socket yet. Thus the outgoing messages need to be prepared to queue
    // up data internally before sending it on further to the socket's queue.
    //
    // This function, outgoingFlush(), is called by both the Server and Client
    // to attempt to flush any pending messages out to the socket.
    _flush() {
      const socket = this.socket;

      if (socket && socket.writable) {
        // There might be remaining data in this.output; write it out
        const ret = this._flushOutput(socket);

        if (this.finished) {
          // This is a queue to the server or client to bring in the next this.
          this._finish();
        } else if (ret && this[kNeedDrain]) {
          this[kNeedDrain] = false;
          this.emit("drain");
        }
      }
    },

    _flushOutput(socket: Socket) {
      while (this[kCorked]) {
        this[kCorked]--;
        socket.cork();
      }

      const outputLength = this.outputData.length;
      if (outputLength <= 0) {
        return undefined;
      }

      const outputData = this.outputData;
      socket.cork();
      let ret;
      // Retain for(;;) loop for performance reasons
      // Refs: https://github.com/nodejs/node/pull/30958
      for (let i = 0; i < outputLength; i++) {
        const { data, encoding, callback } = outputData[i];
        ret = socket.write(data, encoding, callback);
        outputData[i].data = null;
      }
      socket.uncork();

      this.outputData = [];
      this._onPendingData(-this.outputSize);
      this.outputSize = 0;

      return ret;
    },

    /** Right after socket is ready, we need to writeHeader() to setup the request and
     *  client. This is invoked by onSocket(). */
    _flushHeaders() {
      if (!this._headerSent) {
        this._headerSent = true;
        this._writeHeader();
      }
    },

    _send(data: any, encoding?: string | null, callback?: () => void) {
      // Native fast path: an explicit _send() is a manual flush (e.g. test code
      // forcing packet boundaries via res._send('')). Promote to a streaming
      // response so the headers commit now with streaming framing (chunked for
      // HTTP/1.1, close-delimited for HTTP/1.0) instead of coalescing into a
      // flat Content-Length body. The native write()+end() hot path commits via
      // nativeEnd/nativeEndStream and never reaches here, so this only affects
      // code that calls _send() directly.
      if (this[kNativeStream] || this[kNativeExternal]) {
        if (this[kNativeExternal]) {
          nativeStartStream(this);
        }
        if (data !== undefined && data !== null && data.length !== 0) {
          this[kNativeStream].enqueue(nativeChunkToBuffer(data, encoding));
        }
        if (typeof callback === "function") {
          (globalThis as any).process.nextTick(callback);
        }
        return true;
      }
      // This is a shameful hack to get the headers and first body chunk onto
      // the same packet. Future versions of Node are going to take care of
      // this at a lower level and in a more general way.
      if (!this._headerSent && this._header !== null) {
        if (
          typeof data === "string" &&
          (encoding === "utf8" || encoding === "latin1" || !encoding)
        ) {
          data = this._header + data;
        } else {
          const header = this._header;
          ArrayPrototypeUnshift(this.outputData, {
            data: header,
            encoding: "latin1",
            callback: null,
          });
          this.outputSize += header.length;
          this._onPendingData(header.length);
        }
        this._headerSent = true;
      }
      return this._writeRaw(data, encoding, callback);
    },

    _writeHeader() {
      throw new ERR_METHOD_NOT_IMPLEMENTED("_writeHeader()");
    },

    _flushBuffer() {
      const outputLength = this.outputData.length;
      if (outputLength <= 0 || !this.socket || !this._bodyWriter) {
        return undefined;
      }

      const { data, encoding, callback } = ArrayPrototypeShift(
        this.outputData,
      );
      this._flushingBuffer = true;
      let ret;
      try {
        ret = this._writeRaw(data, encoding, callback);
      } finally {
        this._flushingBuffer = false;
      }
      if (this.outputData.length > 0) {
        this.once("drain", this._flushBuffer);
      }

      return ret;
    },

    _writeRaw(
      data: any,
      encoding?: string | null,
      callback?: () => void,
    ) {
      const conn = this.socket;
      if (conn?.destroyed) {
        return false;
      }

      if (typeof encoding === "function") {
        callback = encoding;
        encoding = null;
      }

      if (conn && conn._httpMessage === this && conn.writable) {
        // There might be pending data in the this.output buffer.
        if (this.outputData.length) {
          this._flushOutput(conn);
        }
        this._recordRetryData?.(data, encoding, callback);
        // Directly write to socket.
        return conn.write(data, encoding, callback);
      }
      // Buffer, as long as we're not destroyed.
      ArrayPrototypePush(this.outputData, { data, encoding, callback });
      this.outputSize += data.length;
      this._onPendingData(data.length);
      return this.outputSize < HIGH_WATER_MARK;
    },

    _renderHeaders() {
      if (this._header) {
        throw new ERR_HTTP_HEADERS_SENT("render");
      }

      const headersMap = this[kOutHeaders];

      const headers: any = {};

      if (headersMap !== null) {
        const keys = ObjectKeys(headersMap);
        // Retain for(;;) loop for performance reasons
        // Refs: https://github.com/nodejs/node/pull/30958
        for (let i = 0, l = keys.length; i < l; i++) {
          const key = keys[i];
          headers[headersMap[key][0]] = headersMap[key][1];
        }
      }
      return headers;
    },

    _storeHeader(firstLine: string, headers: any) {
      // firstLine in the case of request is: 'GET /index.html HTTP/1.1\r\n'
      // in the case of response it is: 'HTTP/1.1 200 OK\r\n'
      const state = {
        connection: false,
        contLen: false,
        te: false,
        date: false,
        expect: false,
        trailer: false,
        header: firstLine,
      };

      if (headers) {
        if (headers === this[kOutHeaders]) {
          // kOutHeaders format: { lowercase: [OriginalName, value] }
          // deno-lint-ignore guard-for-in
          for (const key in headers) {
            const entry = headers[key];
            this._storeHeaderEntry(state, entry[0], entry[1]);
          }
        } else if (ArrayIsArray(headers)) {
          if (headers.length && ArrayIsArray(headers[0])) {
            // Array of arrays: [[name, value], ...]
            for (let i = 0; i < headers.length; i++) {
              const entry = headers[i];
              this._storeHeaderEntry(state, entry[0], entry[1]);
            }
          } else {
            // Flat array: [name, value, name, value, ...]
            for (let n = 0; n < headers.length; n += 2) {
              this._storeHeaderEntry(state, headers[n], headers[n + 1]);
            }
          }
        } else {
          // Plain object: { name: value }
          const keys = ObjectKeys(headers);
          for (let i = 0; i < keys.length; i++) {
            const k = keys[i];
            this._storeHeaderEntry(state, k, headers[k]);
          }
        }
      }

      // Date header
      if (this.sendDate && !state.date) {
        state.header += "Date: " + utcDate() + "\r\n";
      }

      // Force the connection to close when the response is a 204 No Content or
      // a 304 Not Modified and the user has set a "Transfer-Encoding: chunked"
      // header.
      //
      // RFC 2616 mandates that 204 and 304 responses MUST NOT have a body but
      // node.js used to send out a zero chunk anyway to accommodate clients
      // that don't have special handling for those responses.
      //
      // It was pointed out that this might confuse reverse proxies to the point
      // of creating security liabilities, so suppress the zero chunk and force
      // the connection to close.
      if (
        this.chunkedEncoding && (this.statusCode === 204 ||
          this.statusCode === 304)
      ) {
        debug(
          this.statusCode + " response should not use chunked encoding," +
            " closing connection.",
        );
        this.chunkedEncoding = false;
        this.shouldKeepAlive = false;
      }

      if (this._removedConnection) {
        this._last = !this.shouldKeepAlive;
      } else if (!state.connection) {
        const shouldSendKeepAlive = this.shouldKeepAlive &&
          (state.contLen || this.useChunkedEncodingByDefault || this.agent);
        if (shouldSendKeepAlive && this.maxRequestsOnConnectionReached) {
          state.header += "Connection: close\r\n";
        } else if (shouldSendKeepAlive) {
          state.header += "Connection: keep-alive\r\n";
          if (this._keepAliveTimeout && this._defaultKeepAlive) {
            const timeoutSeconds = MathFloor(this._keepAliveTimeout / 1000);
            let max = "";
            if (~~this._maxRequestsPerSocket > 0) {
              max = `, max=${this._maxRequestsPerSocket}`;
            }
            state.header += `Keep-Alive: timeout=${timeoutSeconds}${max}\r\n`;
          }
        } else {
          this._last = true;
          state.header += "Connection: close\r\n";
        }
      }

      if (!state.contLen && !state.te) {
        if (!this._hasBody) {
          // Make sure we don't end the 0\r\n\r\n at the end of the message.
          this.chunkedEncoding = false;
        } else if (!this.useChunkedEncodingByDefault) {
          this._last = true;
        } else if (
          !state.trailer &&
          !this._removedContLen &&
          typeof this._contentLength === "number"
        ) {
          state.header += "Content-Length: " + this._contentLength + "\r\n";
        } else if (!this._removedTE) {
          state.header += "Transfer-Encoding: chunked\r\n";
          this.chunkedEncoding = true;
        } else {
          // We should only be able to get here if both Content-Length and
          // Transfer-Encoding are removed by the user.
          // See: test/parallel/test-http-remove-header-stays-removed.js
          debug("Both Content-Length and Transfer-Encoding are removed");

          // We can't keep alive in this case, because with no header info the body
          // is defined as all data until the connection is closed.
          this._last = true;
        }
      }

      // Test non-chunked message does not have trailer header set,
      // message will be terminated by the first empty line after the
      // header fields, regardless of the header fields present in the
      // message, and thus cannot contain a message body or 'trailers'.
      if (this.chunkedEncoding !== true && state.trailer) {
        throw new ERR_HTTP_TRAILER_INVALID();
      }

      const { header } = state;
      this._header = header + "\r\n";

      // Wait until the first body chunk, or close(), is sent to flush,
      // UNLESS we're sending Expect: 100-continue.
      if (state.expect) this._send("");
    },

    _storeHeaderEntry(state: any, field: string, value: any) {
      if (ArrayIsArray(value)) {
        // RFC 6265: join multiple Cookie values with '; '
        if (isCookieField(field)) {
          state.header += field + ": " + ArrayPrototypeJoin(value, "; ") +
            "\r\n";
        } else {
          for (let j = 0; j < value.length; j++) {
            state.header += field + ": " + value[j] + "\r\n";
          }
        }
      } else {
        state.header += field + ": " + value + "\r\n";
      }
      this._matchHeader(state, field, value);
    },

    _matchHeader(
      state: any,
      field: string,
      value: any,
    ) {
      // Ignore lint to keep the code as similar to Nodejs as possible
      // deno-lint-ignore no-this-alias
      const self = this;
      if (field.length < 4 || field.length > 17) {
        return;
      }
      field = StringPrototypeToLowerCase(field);
      switch (field) {
        case "connection":
          state.connection = true;
          self._removedConnection = false;
          if (RE_CONN_CLOSE.exec(value) !== null) {
            self._last = true;
          } else {
            self.shouldKeepAlive = true;
          }
          break;
        case "transfer-encoding":
          state.te = true;
          self._removedTE = false;
          if (RE_TE_CHUNKED.exec(value) !== null) {
            self.chunkedEncoding = true;
          }
          break;
        case "content-length":
          state.contLen = true;
          self._contentLength = +value;
          self._removedContLen = false;
          break;
        case "date":
        case "expect":
        case "trailer":
          state[field] = true;
          break;
        case "keep-alive":
          self._defaultKeepAlive = false;
          break;
      }
    },

    [EE.captureRejectionSymbol](err: any, _event: any) {
      this.destroy(err);
    },

    setHeaders(headers: any) {
      if (this._header) {
        throw new ERR_HTTP_HEADERS_SENT("set");
      }

      if (
        !headers ||
        ArrayIsArray(headers) ||
        typeof headers.keys !== "function" ||
        typeof headers.get !== "function"
      ) {
        throw new ERR_INVALID_ARG_TYPE(
          "headers",
          ["Headers", "Map"],
          headers,
        );
      }

      let cookies = null;
      const iterator = headers[SymbolIterator]();
      while (true) {
        // deno-lint-ignore prefer-primordials
        const { done, value: entry } = iterator.next();
        if (done) {
          break;
        }
        const key = entry[0];
        const value = entry[1];
        if (key === "set-cookie") {
          if (ArrayIsArray(value)) {
            cookies ??= [];
            ArrayPrototypePushApply(cookies, value);
          } else {
            cookies ??= [];
            ArrayPrototypePush(cookies, value);
          }
          continue;
        }
        this.setHeader(key, value);
      }
      if (cookies != null) {
        this.setHeader("set-cookie", cookies);
      }

      return this;
    },
  }),
);

ObjectDefineProperty(OutgoingMessage.prototype, "socket", {
  __proto__: null,
  get: function (this: any) {
    return this[kSocket];
  },
  set: function (this: any, val: any) {
    for (let n = 0; n < this[kCorked]; n++) {
      val?.cork();
      this[kSocket]?.uncork();
    }
    this[kSocket] = val;
  },
});

ObjectDefineProperty(OutgoingMessage.prototype, "_headers", {
  __proto__: null,
  get: deprecate(
    function (this: any) {
      return this.getHeaders();
    },
    "OutgoingMessage.prototype._headers is deprecated",
    "DEP0066",
  ),
  set: deprecate(
    function (this: any, val: any) {
      if (val == null) {
        this[kOutHeaders] = null;
      } else if (typeof val === "object") {
        const headers = this[kOutHeaders] = ObjectCreate(null);
        const keys = ObjectKeys(val);
        // Retain for(;;) loop for performance reasons
        // Refs: https://github.com/nodejs/node/pull/30958
        for (let i = 0; i < keys.length; ++i) {
          const name = keys[i];
          headers[StringPrototypeToLowerCase(name)] = [name, val[name]];
        }
      }
    },
    "OutgoingMessage.prototype._headers is deprecated",
    "DEP0066",
  ),
});

ObjectDefineProperty(OutgoingMessage.prototype, "_headerNames", {
  __proto__: null,
  get: deprecate(
    function (this: any) {
      const headers = this[kOutHeaders];
      if (headers !== null) {
        const out = ObjectCreate(null);
        const keys = ObjectKeys(headers);
        // Retain for(;;) loop for performance reasons
        // Refs: https://github.com/nodejs/node/pull/30958
        for (let i = 0; i < keys.length; ++i) {
          const key = keys[i];
          const val = headers[key][0];
          out[key] = val;
        }
        return out;
      }
      return null;
    },
    "OutgoingMessage.prototype._headerNames is deprecated",
    "DEP0066",
  ),
  set: deprecate(
    function (this: any, val: any) {
      if (typeof val === "object" && val !== null) {
        const headers = this[kOutHeaders];
        if (!headers) {
          return;
        }
        const keys = ObjectKeys(val);
        // Retain for(;;) loop for performance reasons
        // Refs: https://github.com/nodejs/node/pull/30958
        for (let i = 0; i < keys.length; ++i) {
          const header = headers[keys[i]];
          if (header) {
            header[0] = val[keys[i]];
          }
        }
      }
    },
    "OutgoingMessage.prototype._headerNames is deprecated",
    "DEP0066",
  ),
});

export const validateHeaderName = hideStackFrames(
  (name: string, label?: string): void => {
    if (typeof name !== "string" || !name || !checkIsHttpToken(name)) {
      throw new ERR_INVALID_HTTP_TOKEN(label || "Header name", name);
    }
  },
);

export const validateHeaderValue = hideStackFrames(
  (name: string, value: string): void => {
    if (value === undefined) {
      throw new ERR_HTTP_INVALID_HEADER_VALUE(value, name);
    }
    if (checkInvalidHeaderChar(value)) {
      debug('Header "%s" contains invalid characters', name);
      throw new ERR_INVALID_CHAR("header content", name);
    }
  },
);

export function parseUniqueHeadersOption(headers) {
  if (!ArrayIsArray(headers)) {
    return null;
  }

  const unique = new SafeSet();
  const l = headers.length;
  for (let i = 0; i < l; i++) {
    unique.add(StringPrototypeToLowerCase(headers[i]));
  }

  return unique;
}

ObjectDefineProperty(OutgoingMessage.prototype, "headersSent", {
  __proto__: null,
  configurable: true,
  enumerable: true,
  get: function () {
    return !!this._header;
  },
});

// deno-lint-ignore camelcase
const crlf_buf = Buffer.from("\r\n");

function _checkStrictContentLength(msg: any) {
  return (
    msg.strictContentLength &&
    msg._contentLength != null &&
    msg._hasBody &&
    !msg._removedContLen &&
    !msg.chunkedEncoding &&
    !msg.hasHeader("transfer-encoding")
  );
}

function write_(
  msg: any,
  chunk: any,
  encoding: string | null,
  callback: any,
  fromEnd: boolean,
): boolean {
  if (typeof callback !== "function") {
    callback = nop;
  }

  if (chunk === null) {
    throw new ERR_STREAM_NULL_VALUES();
  } else if (typeof chunk !== "string" && !isUint8Array(chunk)) {
    throw new ERR_INVALID_ARG_TYPE(
      "chunk",
      ["string", "Buffer", "Uint8Array"],
      chunk,
    );
  }

  let err;
  if (msg.finished) {
    err = new ERR_STREAM_WRITE_AFTER_END();
  } else if (msg.destroyed) {
    err = new ERR_STREAM_DESTROYED("write");
  }

  if (err) {
    if (!msg.destroyed) {
      _onError(msg, err, callback);
    } else {
      (globalThis as any).process.nextTick(callback, err);
    }
    return false;
  }

  let len;

  if (msg.strictContentLength) {
    len ??= typeof chunk === "string"
      ? Buffer.byteLength(chunk, encoding)
      : TypedArrayPrototypeGetByteLength(chunk);

    if (
      _checkStrictContentLength(msg) &&
      (fromEnd
        ? msg[kBytesWritten] + len !== msg._contentLength
        : msg[kBytesWritten] + len > msg._contentLength)
    ) {
      throw new ERR_HTTP_CONTENT_LENGTH_MISMATCH(
        len + msg[kBytesWritten],
        msg._contentLength,
      );
    }

    msg[kBytesWritten] += len;
  }

  if (!msg._header) {
    if (fromEnd) {
      len ??= typeof chunk === "string"
        ? Buffer.byteLength(chunk, encoding)
        : TypedArrayPrototypeGetByteLength(chunk);
      msg._contentLength = len;
    }
    msg._implicitHeader();
  }

  if (!msg._hasBody) {
    if (msg[kRejectNonStandardBodyWrites]) {
      throw new ERR_HTTP_BODY_NOT_ALLOWED();
    }
    debug(
      "This type of response MUST NOT have a body. " +
        "Ignoring write() calls.",
    );
    (globalThis as any).process.nextTick(callback);
    return true;
  }

  // Auto-corking
  if (!fromEnd && msg.socket && !msg.socket.writableCorked) {
    msg.socket.cork();
    (globalThis as any).process.nextTick(connectionCorkNT, msg.socket);
  }

  let ret;
  if (msg.chunkedEncoding && chunk.length !== 0) {
    len ??= typeof chunk === "string"
      ? Buffer.byteLength(chunk, encoding)
      : TypedArrayPrototypeGetByteLength(chunk);
    msg._send(NumberPrototypeToString(len, 16), "latin1", null);
    msg._send(crlf_buf, null, null);
    msg._send(chunk, encoding, null);
    ret = msg._send(crlf_buf, null, callback);
  } else {
    ret = msg._send(chunk, encoding, callback);
  }

  return ret;
}

function connectionCorkNT(conn: any) {
  conn.uncork();
}

function tryDirectEnd(
  msg: any,
  chunk: any,
  encoding: string | null,
  callback: any,
) {
  if (
    typeof chunk !== "string" ||
    msg.req === undefined ||
    msg.strictContentLength ||
    msg._header !== null ||
    msg.outputData.length !== 0 ||
    msg[kChunkedLength] !== 0 ||
    msg._bodyWriter !== null ||
    !msg._hasBody ||
    msg._trailer !== "" ||
    msg._removedContLen ||
    msg._removedTE ||
    msg.chunkedEncoding ||
    msg.statusCode === 204 ||
    msg.statusCode === 304
  ) {
    return false;
  }

  const normalizedEncoding = normalizeDirectStringEncoding(encoding);
  if (normalizedEncoding === null) {
    return false;
  }

  const headers = msg[kOutHeaders];
  if (
    headers !== null &&
    (headers["transfer-encoding"] !== undefined ||
      headers.trailer !== undefined)
  ) {
    return false;
  }

  const socket = msg.socket;
  const writableState = socket?._writableState;
  if (
    socket == null ||
    socket.destroyed ||
    !socket.writable ||
    socket.connecting ||
    socket._httpMessage !== msg ||
    socket._handle?.isStreamBase !== true ||
    typeof socket._handle.writeUtf8String !== "function" ||
    (normalizedEncoding === "latin1" &&
      typeof socket._handle.writeLatin1String !== "function") ||
    writableState === undefined ||
    writableState.corked !== 0 ||
    writableState.length !== 0 ||
    writableState.needDrain
  ) {
    return false;
  }

  const len = Buffer.byteLength(chunk, normalizedEncoding);
  msg._contentLength = len;
  msg._implicitHeader();

  // The normal write fallback is _header-aware if header generation caused a
  // guard to trip.
  if (!msg._hasBody || msg.chunkedEncoding || msg._header === null) {
    return false;
  }

  if (typeof callback === "function") {
    msg.once("finish", callback);
  }

  const data = msg._header + chunk;
  const finish = FunctionPrototypeBind(onFinish, undefined, msg);
  const result = writeDirectString(socket, data, normalizedEncoding, finish);

  msg._headerSent = true;
  msg.finished = true;

  if (
    msg.outputData.length === 0 &&
    socket._httpMessage === msg
  ) {
    msg._finish();
  }

  if (result === 0) {
    (globalThis as any).process.nextTick(finish);
  } else if (result < 0) {
    socket.destroy(errnoException(result, "write"));
  }

  return true;
}

function tryDirectEmptyEnd(
  msg: any,
  callback: any,
) {
  if (
    msg.req === undefined ||
    msg.strictContentLength ||
    msg._headerSent ||
    msg.outputData.length !== 0 ||
    msg[kChunkedLength] !== 0 ||
    msg._bodyWriter !== null ||
    !msg._hasBody ||
    msg._trailer !== "" ||
    msg._removedContLen ||
    msg._removedTE ||
    msg.chunkedEncoding ||
    msg.statusCode === 204 ||
    msg.statusCode === 304
  ) {
    return false;
  }

  const headers = msg[kOutHeaders];
  if (
    headers !== null &&
    (headers["transfer-encoding"] !== undefined ||
      headers.trailer !== undefined)
  ) {
    return false;
  }

  const socket = msg.socket;
  const writableState = socket?._writableState;
  if (
    socket == null ||
    socket.destroyed ||
    !socket.writable ||
    socket.connecting ||
    socket._httpMessage !== msg ||
    socket._handle?.isStreamBase !== true ||
    typeof socket._handle.writeLatin1String !== "function" ||
    writableState === undefined ||
    writableState.corked !== 0 ||
    writableState.length !== 0 ||
    writableState.needDrain
  ) {
    return false;
  }

  if (msg._header === null) {
    msg._contentLength = 0;
    msg._implicitHeader();
  }

  if (!msg._hasBody || msg.chunkedEncoding || msg._header === null) {
    return false;
  }

  if (typeof callback === "function") {
    msg.once("finish", callback);
  }

  const finish = FunctionPrototypeBind(onFinish, undefined, msg);
  const result = writeDirectString(socket, msg._header, "latin1", finish);

  msg._headerSent = true;
  msg.finished = true;

  if (
    msg.outputData.length === 0 &&
    socket._httpMessage === msg
  ) {
    msg._finish();
  }

  if (result === 0) {
    (globalThis as any).process.nextTick(finish);
  } else if (result < 0) {
    socket.destroy(errnoException(result, "write"));
  }

  return true;
}

function normalizeDirectStringEncoding(encoding: string | null) {
  if (encoding === null || encoding === undefined) {
    return "utf8";
  }

  switch (StringPrototypeToLowerCase(String(encoding))) {
    case "utf8":
    case "utf-8":
      return "utf8";
    case "latin1":
    case "binary":
      return "latin1";
    default:
      return null;
  }
}

function writeDirectString(
  socket: any,
  data: string,
  encoding: string,
  finish: () => void,
) {
  const handle = socket._handle;
  // This terminal response write bypasses stream_base_commons completion
  // bookkeeping; the caller finishes the ServerResponse directly.
  socket._unrefTimer?.();

  const req = {
    oncomplete(status: number) {
      if (status < 0) {
        socket.destroy(errnoException(status, "write"));
        return;
      }
      finish();
    },
  };

  let err;
  switch (encoding) {
    case "latin1":
      err = handle.writeLatin1String(req, data);
      break;
    default:
      err = handle.writeUtf8String(req, data);
      break;
  }

  if (err !== 0) {
    return err;
  }

  return streamBaseState[kLastWriteWasAsync] ? 1 : 0;
}

function onFinish(outmsg: any) {
  if (outmsg?.socket?._hadError) return;
  // If the response (or its socket) was destroyed before `res.end()` ran
  // (e.g. the client aborted the connection), do not emit 'finish' - Node
  // only emits 'close' in that scenario.
  if (outmsg?.destroyed || outmsg?.socket?.destroyed) return;
  outmsg.emit("finish");
}

function _onError(msg: any, err: any, callback: any) {
  const triggerAsyncId = msg.socket ? msg.socket[async_id_symbol] : undefined;
  defaultTriggerAsyncIdScope(
    triggerAsyncId,
    (globalThis as any).process.nextTick,
    emitErrorNt,
    msg,
    err,
    callback,
  );
}

function emitErrorNt(msg: any, err: any, callback: any) {
  callback(err);
  if (typeof msg.emit === "function" && !msg._closed) {
    msg.emit("error", err);
  }
}

export default {
  kUniqueHeaders,
  kHighWaterMark,
  kRejectNonStandardBodyWrites,
  validateHeaderName,
  validateHeaderValue,
  parseUniqueHeadersOption,
  OutgoingMessage,
};
