// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

// Ported from Node.js lib/_http_server.js

import { core, internals, primordials } from "ext:core/mod.js";
import {
  op_get_env_no_permission_check,
  op_http_abort_response,
  op_http_get_request_headers,
  op_http_get_request_http_minor_version,
  op_http_get_request_method,
  op_http_get_request_remote_addr,
  op_http_get_request_trailers,
  op_http_get_request_url,
  op_http_read_request_body,
  op_http_reclaim_socket,
  op_http_request_on_cancel,
  op_http_set_allow_half_open,
  op_http_set_response_interim,
  op_http_set_response_status_message,
  op_http_try_take_full_request_body,
} from "ext:core/ops";
const {
  ArrayIsArray,
  ArrayPrototypeIncludes,
  ArrayPrototypePush,
  ArrayPrototypeShift,
  ArrayPrototypeSlice,
  Error,
  ErrorPrototype,
  FunctionPrototypeApply,
  FunctionPrototypeBind,
  FunctionPrototypeCall,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  MathMin,
  Number,
  NumberIsFinite,
  ObjectDefineProperty,
  ObjectHasOwn,
  ObjectKeys,
  PromisePrototypeThen,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  SafeArrayIterator,
  SafeMap,
  SafeMapIterator,
  SafeSet,
  SafeSetIterator,
  String,
  StringPrototypeIncludes,
  StringPrototypeIndexOf,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  StringPrototypeToLowerCase,
  Symbol,
  SymbolHasInstance,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  Uint8Array,
} = primordials;

import net from "node:net";
import { Duplex } from "node:stream";
import { AsyncResource } from "node:async_hooks";
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { ok: assert } = core.loadExtScript("ext:deno_node/assert.ts");
const { enabledHooksExist } = core.loadExtScript(
  "ext:deno_node/internal/async_hooks.ts",
);
import {
  _checkInvalidHeaderChar as checkInvalidHeaderChar,
  chunkExpression,
  continueExpression,
  freeParser,
  HTTPParser,
  isLenient,
  kIncomingMessage,
  parsers,
  prepareError,
} from "node:_http_common";
import {
  kUniqueHeaders,
  OutgoingMessage,
  parseUniqueHeadersOption,
  validateHeaderName,
  validateHeaderValue,
} from "node:_http_outgoing";
const { kNativeExternal, kNeedDrain, kOutHeaders } = core
  .loadExtScript(
    "ext:deno_node/internal/http.ts",
  );
import { IncomingMessage } from "node:_http_incoming";
const {
  connResetException,
  ERR_HTTP_HEADERS_SENT,
  ERR_HTTP_TRAILER_INVALID,
  ERR_HTTP_SOCKET_ASSIGNED,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  ERR_INVALID_CHAR,
  ERR_OUT_OF_RANGE,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { kEmptyObject } = core.loadExtScript("ext:deno_node/internal/util.mjs");
const {
  kDestroy,
  kTimeout,
  suspendTimeout,
} = core.loadExtScript("ext:deno_node/internal/timers.mjs");
const {
  validateBoolean,
  validateFunction,
  validateInteger,
  validateLinkHeaderValue,
  validateObject,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { nextTick } = core.loadExtScript("ext:deno_node/_next_tick.ts");
const {
  enterAsyncResourceIfActive,
  exitAsyncResourceIfActive,
} = core.loadExtScript("ext:deno_node/internal/async_hooks.ts");
const { enqueueNodePerformanceEntry, hasNodeObserverForType } = core
  .loadExtScript(
    "ext:deno_node/perf_hooks.js",
  );
import {
  applyAddressOverride,
  notifyAddressOverrideServing,
  startOverrideListener,
} from "ext:deno_node/internal/http/address_override.js";
const {
  otelState,
  builtinTracer,
  ContextManager,
  telemetry,
} = core.loadExtScript("ext:deno_telemetry/telemetry.ts");
const { channel } = core.loadExtScript("ext:deno_node/diagnostics_channel.js");

const onServerRequestStartChannel = channel("http.server.request.start");
const onServerResponseCreatedChannel = channel("http.server.response.created");
const onServerResponseFinishChannel = channel("http.server.response.finish");

const kServerResponse = Symbol("ServerResponse");
const kConnectionsKey = Symbol("http.server.connections");
const kConnectionsCheckingInterval = Symbol(
  "http.server.connectionsCheckingInterval",
);
const kOtelSpan = Symbol("kOtelSpan");
const kOtelStartTime = Symbol("kOtelStartTime");
const kOtelReqBodySize = Symbol("kOtelReqBodySize");
const kPerfStartTime = Symbol("kPerfStartTime");

// OTel server metrics - lazy initialized
let otelMetrics = null;

function getOtelMetrics() {
  if (otelMetrics) return otelMetrics;
  const meter = telemetry.meterProvider.getMeter("deno.http.server");
  otelMetrics = {
    activeRequests: meter.createUpDownCounter("http.server.active_requests", {
      description: "Number of active HTTP server requests.",
      unit: "{request}",
    }),
    requestDuration: meter.createHistogram("http.server.request.duration", {
      description: "Duration of HTTP server requests.",
      unit: "s",
      advice: {
        explicitBucketBoundaries: [
          0.005,
          0.01,
          0.025,
          0.05,
          0.075,
          0.1,
          0.25,
          0.5,
          0.75,
          1,
          2.5,
          5,
          7.5,
          10,
        ],
      },
    }),
    requestBodySize: meter.createHistogram("http.server.request.body.size", {
      description: "Size of HTTP server request bodies.",
      unit: "By",
    }),
    responseBodySize: meter.createHistogram("http.server.response.body.size", {
      description: "Size of HTTP server response bodies.",
      unit: "By",
    }),
  };
  return otelMetrics;
}
const kLenientAll = HTTPParser.kLenientAll | 0;
const kLenientNone = HTTPParser.kLenientNone | 0;
const kOnExecute = HTTPParser.kOnExecute | 0;
const kOnMessageBegin = HTTPParser.kOnMessageBegin | 0;
const _kOnTimeout = HTTPParser.kOnTimeout | 0;

// JS-based ConnectionsList matching Node's native ConnectionsList.
// Tracks active connections and their request start times for
// headersTimeout / requestTimeout enforcement.
class ConnectionsList {
  constructor() {
    this._all = new SafeSet();
    this._active = new SafeMap(); // socket -> { headersCompleted, startTime, req }
  }

  add(socket) {
    this._all.add(socket);
  }

  remove(socket) {
    this._all.delete(socket);
    this._active.delete(socket);
  }

  pushActive(socket) {
    this._active.set(socket, {
      headersCompleted: false,
      startTime: performance.now(),
      req: null,
    });
  }

  popActive(socket) {
    this._active.delete(socket);
  }

  // For pipelined requests the parser fires kOnMessageBegin for the next
  // request before the previous response finishes, replacing the active
  // entry. resOnFinish must only clear the entry if it still tracks the
  // request whose response just finished, not a pipelined successor.
  popActiveIfReq(socket, req) {
    const entry = this._active.get(socket);
    if (entry && entry.req === req) {
      this._active.delete(socket);
    }
  }

  markHeadersCompleted(socket, req) {
    const entry = this._active.get(socket);
    if (entry) {
      entry.headersCompleted = true;
      entry.req = req;
    }
  }

  expired(headersTimeout, requestTimeout) {
    const now = performance.now();
    const result = [];

    for (const { 0: socket, 1: entry } of new SafeMapIterator(this._active)) {
      const elapsed = now - entry.startTime;
      if (!entry.headersCompleted && headersTimeout > 0) {
        if (elapsed >= headersTimeout) {
          ArrayPrototypePush(result, { socket });
          continue;
        }
      }
      if (requestTimeout > 0 && elapsed >= requestTimeout) {
        // requestTimeout only covers receiving the entire request from the
        // client. Once the full request message has been parsed off the wire
        // (req.complete is set in parserOnMessageComplete), the clock stops,
        // mirroring Node resetting last_message_start_ in on_message_complete.
        // Without this an actively streaming response (SSE/proxy) gets aborted
        // at requestTimeout even though the request was long since received.
        if (entry.req?.complete) {
          continue;
        }
        ArrayPrototypePush(result, { socket });
      }
    }

    return result;
  }
}

function onRequestTimeout(socket) {
  const err = new Error("ERR_HTTP_REQUEST_TIMEOUT");
  err.code = "ERR_HTTP_REQUEST_TIMEOUT";
  FunctionPrototypeCall(socketOnError, socket, err);
}

const STATUS_CODES = {
  100: "Continue",
  101: "Switching Protocols",
  102: "Processing",
  103: "Early Hints",
  200: "OK",
  201: "Created",
  202: "Accepted",
  203: "Non-Authoritative Information",
  204: "No Content",
  205: "Reset Content",
  206: "Partial Content",
  207: "Multi-Status",
  208: "Already Reported",
  226: "IM Used",
  300: "Multiple Choices",
  301: "Moved Permanently",
  302: "Found",
  303: "See Other",
  304: "Not Modified",
  305: "Use Proxy",
  307: "Temporary Redirect",
  308: "Permanent Redirect",
  400: "Bad Request",
  401: "Unauthorized",
  402: "Payment Required",
  403: "Forbidden",
  404: "Not Found",
  405: "Method Not Allowed",
  406: "Not Acceptable",
  407: "Proxy Authentication Required",
  408: "Request Timeout",
  409: "Conflict",
  410: "Gone",
  411: "Length Required",
  412: "Precondition Failed",
  413: "Payload Too Large",
  414: "URI Too Long",
  415: "Unsupported Media Type",
  416: "Range Not Satisfiable",
  417: "Expectation Failed",
  418: "I'm a Teapot",
  421: "Misdirected Request",
  422: "Unprocessable Entity",
  423: "Locked",
  424: "Failed Dependency",
  425: "Too Early",
  426: "Upgrade Required",
  428: "Precondition Required",
  429: "Too Many Requests",
  431: "Request Header Fields Too Large",
  451: "Unavailable For Legal Reasons",
  500: "Internal Server Error",
  501: "Not Implemented",
  502: "Bad Gateway",
  503: "Service Unavailable",
  504: "Gateway Timeout",
  505: "HTTP Version Not Supported",
  506: "Variant Also Negotiates",
  507: "Insufficient Storage",
  508: "Loop Detected",
  509: "Bandwidth Limit Exceeded",
  510: "Not Extended",
  511: "Network Authentication Required",
};

// ---- ServerResponse ----

function ServerResponse(req, options) {
  FunctionPrototypeCall(OutgoingMessage, this, options);

  if (req.method === "HEAD") this._hasBody = false;

  this.req = req;
  this.sendDate = true;
  this._sent100 = false;
  this._expect_continue = false;

  if (req.httpVersionMajor < 1 || req.httpVersionMinor < 1) {
    this.useChunkedEncodingByDefault = chunkExpression.test(
      req.headers?.te,
    );
    this.shouldKeepAlive = false;
  }

  if (onServerResponseCreatedChannel.hasSubscribers) {
    onServerResponseCreatedChannel.publish({
      request: req,
      response: this,
    });
  }
}
ObjectSetPrototypeOf(ServerResponse.prototype, OutgoingMessage.prototype);
ObjectSetPrototypeOf(ServerResponse, OutgoingMessage);

ServerResponse.prototype.statusCode = 200;
ServerResponse.prototype.statusMessage = undefined;

function onServerResponseClose() {
  if (this._httpMessage) {
    emitCloseNT(this._httpMessage);
  }
}

ServerResponse.prototype.assignSocket = function assignSocket(socket) {
  if (socket._httpMessage) {
    throw new ERR_HTTP_SOCKET_ASSIGNED();
  }
  socket._httpMessage = this;
  if (socket._parent) {
    socket._parent._httpMessage = this;
    socket._parent._httpMessageDetached = false;
  }
  socket.on("close", onServerResponseClose);
  this.socket = socket;
  this.emit("socket", socket);
  this._flush();
};

ServerResponse.prototype.detachSocket = function detachSocket(socket) {
  assert(socket._httpMessage === this);
  socket.removeListener("close", onServerResponseClose);
  socket._httpMessage = null;
  socket._httpMessageDetached = true;
  if (socket._parent) {
    socket._parent._httpMessage = null;
    socket._parent._httpMessageDetached = true;
  }
  this.socket = null;
};

// Native fast path: 1xx interim responses (writeContinue/writeProcessing/
// writeInformation/writeEarlyHints) can't reach the wire via the synthetic
// socket's _writeRaw, so hand the raw bytes to the engine, which flushes them
// ahead of the final response. Returns true when handled natively.
function nativeWriteInterim(msg, head, cb) {
  const external = msg[kNativeExternal];
  if (external === null || external === undefined) {
    return false;
  }
  op_http_set_response_interim(external, head);
  if (typeof cb === "function") {
    nextTick(cb);
  }
  return true;
}

ServerResponse.prototype.writeContinue = function writeContinue(cb) {
  if (!nativeWriteInterim(this, "HTTP/1.1 100 Continue\r\n\r\n", cb)) {
    this._writeRaw("HTTP/1.1 100 Continue\r\n\r\n", "ascii", cb);
  }
  this._sent100 = true;
};

ServerResponse.prototype.writeProcessing = function writeProcessing(cb) {
  if (!nativeWriteInterim(this, "HTTP/1.1 102 Processing\r\n\r\n", cb)) {
    this._writeRaw("HTTP/1.1 102 Processing\r\n\r\n", "ascii", cb);
  }
};

ServerResponse.prototype.writeInformation = function writeInformation(
  statusCode,
  headers,
  cb,
) {
  if (this._header) {
    throw new ERR_HTTP_HEADERS_SENT("write");
  }

  if (typeof headers === "function") {
    cb = headers;
    headers = undefined;
  }

  validateInteger(statusCode, "statusCode", 100, 199);

  let head = `HTTP/1.1 ${statusCode} ${
    STATUS_CODES[statusCode] || "unknown"
  }\r\n`;

  if (headers !== null && headers !== undefined) {
    const keys = ObjectKeys(headers);
    for (let i = 0; i < keys.length; i++) {
      const key = keys[i];
      validateHeaderName(key);
      const value = headers[key];
      validateHeaderValue(key, value);
      head += key + ": " + value + "\r\n";
    }
  }

  head += "\r\n";

  if (!nativeWriteInterim(this, head, cb)) {
    this._writeRaw(head, "ascii", cb);
  }
};

ServerResponse.prototype.writeEarlyHints = function writeEarlyHints(
  hints,
  cb,
) {
  let head = "HTTP/1.1 103 Early Hints\r\n";

  validateObject(hints, "hints");

  if (hints.link === null || hints.link === undefined) {
    return;
  }

  const link = validateLinkHeaderValue(hints.link);

  if (link.length === 0) {
    return;
  }

  if (checkInvalidHeaderChar(link)) {
    throw new ERR_INVALID_CHAR("header content", "Link");
  }

  head += "Link: " + link + "\r\n";

  const keys = ObjectKeys(hints);
  for (let i = 0; i < keys.length; i++) {
    const key = keys[i];
    if (key !== "link") {
      validateHeaderName(key);
      const value = hints[key];
      validateHeaderValue(key, value);
      head += key + ": " + value + "\r\n";
    }
  }

  head += "\r\n";

  if (!nativeWriteInterim(this, head, cb)) {
    this._writeRaw(head, "ascii", cb);
  }
};

ServerResponse.prototype._implicitHeader = function _implicitHeader() {
  this.writeHead(this.statusCode);
};

ServerResponse.prototype.writeHead = function writeHead(
  statusCode,
  reason,
  obj,
) {
  if (this._header) {
    throw new ERR_HTTP_HEADERS_SENT("write");
  }

  // The handler assigned an own `res.socket.write` before committing -> it wants
  // raw socket writes to reach the wire. Demote to classic mode so the response
  // flows through res.socket. Rare: the hot path never sets an own `write` on
  // the synthetic socket, so this check is a cheap miss there.
  if (
    this[kNativeExternal] &&
    this.socket !== null && this.socket !== undefined &&
    ObjectHasOwn(this.socket, "write")
  ) {
    demoteNativeResponse(this);
  }

  statusCode |= 0;
  if (statusCode < 100 || statusCode > 999) {
    throw new ERR_INVALID_ARG_TYPE(
      "statusCode",
      "integer [100, 999]",
      statusCode,
    );
  }

  if (typeof reason === "string") {
    this.statusMessage = reason;
  } else {
    this.statusMessage ||= STATUS_CODES[statusCode] || "unknown";
    obj ??= reason;
  }
  this.statusCode = statusCode;

  // Enforce no body for 204 and 304 responses
  if (statusCode === 204 || statusCode === 304) {
    this._hasBody = false;
  }

  // Native mode: force headers into kOutHeaders (below) so they survive into
  // the native commit, rather than only the raw _storeHeader serialization.
  if (this[kNativeExternal] && this[kOutHeaders] === null && obj) {
    this[kOutHeaders] = { __proto__: null };
  }

  let headers;
  if (this[kOutHeaders]) {
    // Slow-case: progressive API and header fields are passed.
    if (ArrayIsArray(obj)) {
      if (this[kNativeExternal] && obj.length && ArrayIsArray(obj[0])) {
        // Native mode forces kOutHeaders above, so the `[[name, value], ...]`
        // tuple form that the classic path hands straight to _storeHeader lands
        // here instead. Handle it like _storeHeader does (remove-then-append so
        // duplicates such as multiple set-cookie headers are preserved).
        for (let n = 0; n < obj.length; n++) {
          const k = obj[n][0];
          if (k) this.removeHeader(k);
        }
        for (let n = 0; n < obj.length; n++) {
          const k = obj[n][0];
          if (k) this.appendHeader(k, obj[n][1]);
        }
      } else {
        if (obj.length % 2 !== 0) {
          throw new ERR_INVALID_ARG_VALUE("headers", obj);
        }
        for (let n = 0; n < obj.length; n += 2) {
          const k = obj[n + 0];
          if (k) this.removeHeader(k);
        }
        for (let n = 0; n < obj.length; n += 2) {
          const k = obj[n + 0];
          if (k) this.appendHeader(k, obj[n + 1]);
        }
      }
    } else if (obj) {
      const keys = ObjectKeys(obj);
      for (let i = 0; i < keys.length; i++) {
        const k = keys[i];
        if (k) this.setHeader(k, obj[k]);
      }
    }
    headers = this[kOutHeaders];
  } else {
    // Only writeHead() called - pass raw headers to _storeHeader
    headers = obj;
  }

  if (checkInvalidHeaderChar(this.statusMessage)) {
    throw new ERR_INVALID_CHAR("statusMessage");
  }

  const statusLine = "HTTP/1.1 " + statusCode + " " + this.statusMessage +
    "\r\n";
  // Native mode: skip socket-oriented header serialization. The headers are in
  // kOutHeaders (forced above) and the native engine adds Date/Content-Length/
  // Connection itself; nativeEnd() reads kOutHeaders on commit.
  if (this[kNativeExternal]) {
    // A Trailer header is only valid with chunked transfer-encoding; a
    // Content-Length response is non-chunked, so trailers can't be sent (Node
    // throws this from _storeHeader, which writeHead invokes).
    if (this.hasHeader("trailer") && this.hasHeader("content-length")) {
      throw new ERR_HTTP_TRAILER_INVALID();
    }
    this._header = "\r\n"; // truthy sentinel -> headersSent true; never written
    // writeHead resolved statusMessage; mark it so nativeCommit doesn't also try
    // (it handles the no-writeHead `res.statusMessage = ...; res.end()` case).
    this._nativeWriteHead = true;
    // Node commits the status into the header at writeHead time; a later
    // `res.statusCode = ...` then has no effect. Native commit is deferred to
    // end(), so lock the status here for it to read.
    this._nativeStatus = statusCode;
    // Only push a custom reason phrase to the engine (it uses the canonical one
    // by default); avoids a per-response op on the hot path.
    if (this.statusMessage !== (STATUS_CODES[statusCode] || "unknown")) {
      op_http_set_response_status_message(
        this[kNativeExternal],
        this.statusMessage,
      );
    }
    return this;
  }

  this._storeHeader(statusLine, headers);

  return this;
};

function emitCloseNT(self) {
  self.destroyed = true;
  self._closed = true;
  self.emit("close");
}

// ---- Server ----

function connectionListener(socket) {
  connectionListenerInternal(this, socket);
}

function connectionListenerInternal(server, socket) {
  socket.server = server;

  // Track connections via ConnectionsList for timeout enforcement
  if (!server[kConnectionsKey]) server[kConnectionsKey] = new ConnectionsList();
  const connections = server[kConnectionsKey];
  connections.add(socket);
  const onConnectionClose = () => connections.remove(socket);
  socket.on("close", onConnectionClose);

  if (server.timeout && typeof socket.setTimeout === "function") {
    socket.setTimeout(server.timeout);
  }
  socket.on("timeout", socketOnTimeout);

  const parser = parsers.alloc();

  const lenient = server.insecureHTTPParser === undefined
    ? isLenient()
    : server.insecureHTTPParser;

  parser.initialize(
    HTTPParser.REQUEST,
    {},
    server.maxHeaderSize || 0,
    lenient ? kLenientAll : kLenientNone,
  );
  parser.socket = socket;
  socket.parser = parser;

  if (typeof server.maxHeadersCount === "number") {
    parser.maxHeaderPairs = server.maxHeadersCount << 1;
  }

  const state = {
    onData: null,
    onEnd: null,
    onClose: null,
    onConnectionClose: onConnectionClose,
    onDrain: null,
    outgoing: [],
    incoming: [],
    outgoingData: 0,
    requestsCount: 0,
    keepAliveTimeoutSet: false,
    keepAliveTimeout: null,
    keepAliveTimeoutMsecs: 0,
    keepAliveTimeoutSuspended: false,
  };
  state.onData = FunctionPrototypeBind(
    socketOnData,
    undefined,
    server,
    socket,
    parser,
    state,
  );
  state.onEnd = FunctionPrototypeBind(
    socketOnEnd,
    undefined,
    server,
    socket,
    parser,
    state,
  );
  state.onClose = FunctionPrototypeBind(
    socketOnClose,
    undefined,
    socket,
    state,
  );
  state.onDrain = FunctionPrototypeBind(
    socketOnDrain,
    undefined,
    socket,
    state,
  );
  socket.on("data", state.onData);
  socket.on("error", socketOnError);
  socket.on("end", state.onEnd);
  socket.on("close", state.onClose);
  socket.on("drain", state.onDrain);
  parser.onIncoming = FunctionPrototypeBind(
    parserOnIncoming,
    undefined,
    server,
    socket,
    state,
  );

  // Mark this connection as active for timeout tracking
  connections.pushActive(socket);

  // Try to consume the socket handle for direct parser reads
  if (
    socket._handle?.isStreamBase &&
    !socket._handle._consumed
  ) {
    parser._consumed = true;
    socket._handle._consumed = true;
    parser.consume(socket._handle);
  }
  parser[kOnExecute] = FunctionPrototypeBind(
    onParserExecute,
    undefined,
    server,
    socket,
    parser,
    state,
  );
  // Reset timeout-tracking state at the start of each HTTP message so
  // headersTimeout/requestTimeout are measured per-request on keepalive
  // connections (mirrors Node's native on_message_begin behavior).
  parser[kOnMessageBegin] = FunctionPrototypeBind(
    onParserMessageBegin,
    undefined,
    server,
    socket,
  );

  socket._paused = false;
}

function onParserMessageBegin(server, socket) {
  const connections = server[kConnectionsKey];
  if (connections) {
    connections.popActive(socket);
    connections.pushActive(socket);
  }
}

function onParserExecute(server, socket, parser, state, ret, d) {
  // Don't refresh the socket timeout while the connection is idling in
  // keep-alive mode (waiting for the next request). Stray bytes like
  // `\r\n` between requests must not reset keepAliveTimeout. The timer
  // is reset explicitly via resetSocketTimeout once a new request
  // actually begins.
  if (!state.keepAliveTimeoutSet) {
    socket._unrefTimer?.();
  }
  // The consume path (parser.consume(handle)) passes `d` as a bare
  // Uint8Array from the C++ binding. onParserExecuteCommon's upgrade
  // branch does `d.slice(bytesParsed).toString()` and expects the
  // Buffer `.toString(encoding)` semantics, not the plain Uint8Array
  // behavior. Wrap to match the non-consume path where `d` came from
  // `socket.on('data')` as a Buffer.
  if (d !== undefined && !Buffer.isBuffer(d)) {
    d = Buffer.from(
      TypedArrayPrototypeGetBuffer(d),
      TypedArrayPrototypeGetByteOffset(d),
      TypedArrayPrototypeGetByteLength(d),
    );
  }
  if (d !== undefined) {
    parser._lastRawPacket = d;
  }
  onParserExecuteCommon(server, socket, parser, state, ret, d);
}

function socketOnTimeout() {
  const req = this.parser?.incoming;
  const reqTimeout = req && !req.complete && req.emit("timeout", this);
  const res = this._httpMessage;
  const resTimeout = res && res.emit("timeout", this);
  const serverTimeout = this.server.emit("timeout", this);
  if (!reqTimeout && !resTimeout && !serverTimeout) {
    this.destroy();
  }
}

function socketOnClose(socket, state) {
  destroySuspendedKeepAliveTimeout(state);
  freeParser(socket.parser, undefined, socket);
  abortIncoming(state.incoming);
}

function abortIncoming(incoming) {
  while (incoming.length) {
    const req = ArrayPrototypeShift(incoming);
    req.destroy(connResetException("aborted"));
  }
}

function socketOnEnd(server, socket, parser, state) {
  const ret = parser.finish();

  if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, ret)) {
    prepareError(ret, parser, parser._lastRawPacket);
    FunctionPrototypeCall(socketOnError, socket, ret);
    return;
  }

  if (!server.httpAllowHalfOpen) {
    abortIncoming(state.incoming);
    if (socket.writable) socket.end();
  } else if (state.outgoing.length) {
    state.outgoing[state.outgoing.length - 1]._last = true;
  } else if (socket._httpMessage) {
    socket._httpMessage._last = true;
  } else if (socket.writable) {
    socket.end();
  }
}

function socketOnData(server, socket, parser, state, d) {
  assert(googLength(d));

  parser._lastRawPacket = d;
  const ret = parser.execute(d);

  onParserExecuteCommon(server, socket, parser, state, ret, d);
}

function googLength(d) {
  return d.length || TypedArrayPrototypeGetByteLength(d);
}

function onParserExecuteCommon(server, socket, parser, state, ret, d) {
  resetSocketTimeout(server, socket, state);

  if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, ret)) {
    prepareError(ret, parser, d);
    FunctionPrototypeCall(socketOnError, socket, ret);
    return;
  }

  // If the parser is paused after headers, unpause on next tick
  if (parser.incoming?.upgrade) {
    const bytesParsed = ret;
    const req = parser.incoming;

    const eventName = req.method === "CONNECT" ? "connect" : "upgrade";
    if (server.listenerCount(eventName) === 0) {
      socket.destroy();
      return;
    }

    // Detach the socket from the server by removing all server-added listeners.
    // After this point the socket is fully owned by the connect/upgrade handler.
    socket.removeListener("data", state.onData);
    socket.removeListener("end", state.onEnd);
    socket.removeListener("close", state.onClose);
    socket.removeListener("close", state.onConnectionClose);
    socket.removeListener("drain", state.onDrain);
    socket.removeListener("error", socketOnError);
    socket.removeListener("timeout", socketOnTimeout);
    // Remove from connection tracking (normally done by the close listener)
    const connections = server[kConnectionsKey];
    if (connections) connections.remove(socket);

    parser.finish();
    freeParser(parser, req, socket);

    // deno-lint-ignore prefer-primordials -- d is a Node Buffer; Buffer.prototype.slice returns a Buffer view
    const bodyHead = d.slice(bytesParsed);

    socket.readableFlowing = null;
    server.emit(eventName, req, socket, bodyHead);
  }
}

function resetSocketTimeout(server, socket, state, allowKeepAliveReuse = true) {
  if (!state.keepAliveTimeoutSet) return;

  const keepAliveTimeout = state.keepAliveTimeout;
  if (
    allowKeepAliveReuse &&
    server.timeout === 0 &&
    socket.setTimeout === net.Socket.prototype.setTimeout &&
    socket[kTimeout] === keepAliveTimeout &&
    ArrayPrototypeIncludes(socket.listeners("timeout"), socketOnTimeout)
  ) {
    suspendTimeout(keepAliveTimeout);
    socket[kTimeout] = null;
    socket.timeout = 0;
    state.keepAliveTimeoutSet = false;
    state.keepAliveTimeoutSuspended = true;
    return;
  }

  socket.setTimeout(server.timeout || 0);
  state.keepAliveTimeoutSet = false;
  state.keepAliveTimeout = null;
  state.keepAliveTimeoutMsecs = 0;
  state.keepAliveTimeoutSuspended = false;
}

function socketOnDrain(socket, state) {
  const needPause = state.outgoingData > socket.writableHighWaterMark;
  if (socket._paused && !needPause) {
    socket._paused = false;
    if (socket.parser) {
      socket.resume();
    }
  }

  const msg = socket._httpMessage;
  if (msg && !msg.finished && msg[kNeedDrain]) {
    msg[kNeedDrain] = false;
    msg.emit("drain");
  }
}

function destroySuspendedKeepAliveTimeout(state) {
  if (!state.keepAliveTimeoutSuspended) return;
  const keepAliveTimeout = state.keepAliveTimeout;
  if (keepAliveTimeout !== null) {
    keepAliveTimeout[kDestroy]();
  }
  state.keepAliveTimeout = null;
  state.keepAliveTimeoutMsecs = 0;
  state.keepAliveTimeoutSuspended = false;
}

const badRequestResponse =
  "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n";
const requestHeaderFieldsTooLargeResponse =
  "HTTP/1.1 431 Request Header Fields Too Large\r\nConnection: close\r\n\r\n";
const requestTimeoutResponse =
  "HTTP/1.1 408 Request Timeout\r\nConnection: close\r\n\r\n";

function socketOnError(e) {
  // Ignore further errors
  this.removeListener("error", socketOnError);
  this.on("error", noop);

  if (this.parser) {
    this.parser.finish();
    freeParser(this.parser, undefined, this);
  }

  if (!this.server.emit("clientError", e, this)) {
    // No clientError listener - send an appropriate error response
    if (
      this.writable &&
      (!this._httpMessage || !this._httpMessage._headerSent)
    ) {
      let response;
      switch (e.code) {
        case "HPE_HEADER_OVERFLOW":
          response = requestHeaderFieldsTooLargeResponse;
          break;
        case "ERR_HTTP_REQUEST_TIMEOUT":
          response = requestTimeoutResponse;
          break;
        default:
          response = badRequestResponse;
          break;
      }
      this.write(response);
    }
    this.destroy(e);
  }
}

function noop() {}

function updateOutgoingData(socket, state, delta) {
  state.outgoingData += delta;
  if (
    socket._paused &&
    state.outgoingData < socket.writableHighWaterMark
  ) {
    return;
  }
  socketOnDrain(socket, state);
}

// ---- parserOnIncoming: creates ServerResponse, emits 'request' ----

function parserOnIncoming(server, socket, state, req, keepAlive) {
  // Connections accepted via the DENO_SERVE_ADDRESS override carry
  // absolute-form request targets (the control plane forwards the full
  // public URL, which is how Deno.serve() reconstructs request.url).
  // Node applications expect origin-form, so strip scheme and authority.
  if (
    socket.isDenoServeAddressOverride &&
    (StringPrototypeStartsWith(req.url, "http://") ||
      StringPrototypeStartsWith(req.url, "https://"))
  ) {
    const schemeEnd = StringPrototypeIndexOf(req.url, "://") + 3;
    const pathStart = StringPrototypeIndexOf(req.url, "/", schemeEnd);
    req.url = pathStart === -1 ? "/" : StringPrototypeSlice(req.url, pathStart);
  }

  resetSocketTimeout(server, socket, state, !req.upgrade);

  // Headers have been fully parsed; clear the headersTimeout watchdog and
  // bind the active entry to this request so resOnFinish can identify it
  // even if a pipelined successor has already replaced it.
  const connections = server[kConnectionsKey];
  if (connections) connections.markHeadersCompleted(socket, req);

  if (req.upgrade && req.method !== "CONNECT") {
    if (
      server.shouldUpgradeCallback !== undefined &&
      !server.shouldUpgradeCallback(req)
    ) {
      req.upgrade = false;
    }
  }

  if (req.upgrade) {
    req.upgrade = req.method === "CONNECT" || true;
    if (req.upgrade) return 0;
  }

  ArrayPrototypePush(state.incoming, req);
  if (hasNodeObserverForType("http")) {
    req[kPerfStartTime] = performance.now();
  }

  if (!socket._paused) {
    const ws = socket._writableState;
    if (
      ws?.needDrain || state.outgoingData >= socket.writableHighWaterMark
    ) {
      socket._paused = true;
      socket.pause();
    }
  }

  const res = new server[kServerResponse](req, {
    rejectNonStandardBodyWrites: server.rejectNonStandardBodyWrites,
    highWaterMark: server.highWaterMark,
  });
  res._keepAliveTimeout = server.keepAliveTimeout;
  res._maxRequestsPerSocket = server.maxRequestsPerSocket;
  res._onPendingData = FunctionPrototypeBind(
    updateOutgoingData,
    undefined,
    socket,
    state,
  );

  res.shouldKeepAlive = keepAlive;
  res[kUniqueHeaders] = server[kUniqueHeaders];

  if (server.optimizeEmptyRequests && isRequestKnownEmpty(req)) {
    req._dumpAndCloseReadable();
  }

  // Start OTel server span and metrics
  if (otelState.TRACING_ENABLED) {
    // Extract trace context from incoming request headers
    let context = ContextManager.active();
    if (otelState.PROPAGATORS.length > 0) {
      for (
        const propagator of new SafeArrayIterator(otelState.PROPAGATORS)
      ) {
        context = propagator.extract(context, req.headers, {
          get(carrier, key) {
            return carrier[key];
          },
          keys(carrier) {
            return ObjectKeys(carrier);
          },
        });
      }
    }
    // Create server span within extracted context, without modifying async context
    const span = builtinTracer().startSpan(req.method, { kind: 1 }, context); // SpanKind.SERVER = 1
    const url = req.url || "/";
    const host = req.headers?.host || "localhost";
    const scheme = socket.encrypted ? "https" : "http";
    span.setAttribute("http.request.method", req.method);
    span.setAttribute("url.full", `${scheme}://${host}${url}`);
    span.setAttribute("url.scheme", scheme);
    span.setAttribute("url.path", StringPrototypeSplit(url, "?")[0]);
    span.setAttribute(
      "url.query",
      StringPrototypeIncludes(url, "?")
        ? StringPrototypeSplit(url, "?")[1]
        : "",
    );
    res[kOtelSpan] = span;
  }
  if (otelState.METRICS_ENABLED) {
    res[kOtelStartTime] = performance.now();
    res[kOtelReqBodySize] = 0;
    const metrics = getOtelMetrics();
    const scheme = socket.encrypted ? "https" : "http";
    metrics.activeRequests.add(1, {
      "http.request.method": req.method,
      "url.scheme": scheme,
    });
  }

  if (socket._httpMessage) {
    ArrayPrototypePush(state.outgoing, res);
  } else {
    res.assignSocket(socket);
  }

  res.on(
    "finish",
    FunctionPrototypeBind(
      resOnFinish,
      undefined,
      req,
      res,
      socket,
      state,
      server,
    ),
  );

  if (onServerRequestStartChannel.hasSubscribers) {
    onServerRequestStartChannel.publish({
      request: req,
      response: res,
      socket,
      server,
    });
  }

  // Enter a new async-hooks resource scope for the duration of the request
  // emission. Each request gets its own resource (the IncomingMessage), so
  // executionAsyncResource() returns a per-request object that is preserved
  // across timers, await transitions, etc. Without this, every request would
  // share the top-level resource and concurrent requests would race on any
  // state stashed there.
  const prevAsyncResource = enterAsyncResourceIfActive(req);
  try {
    let handled = false;

    if (req.httpVersionMajor === 1 && req.httpVersionMinor === 1) {
      if (
        server.requireHostHeader !== false &&
        req.headers.host === undefined
      ) {
        res.writeHead(400, ["Connection", "close"]);
        res.end();
        return 0;
      }

      const isRequestsLimitSet =
        typeof server.maxRequestsPerSocket === "number" &&
        server.maxRequestsPerSocket > 0;

      if (isRequestsLimitSet) {
        state.requestsCount++;
        res.maxRequestsOnConnectionReached =
          server.maxRequestsPerSocket <= state.requestsCount;
      }

      if (
        isRequestsLimitSet &&
        server.maxRequestsPerSocket < state.requestsCount
      ) {
        handled = true;
        server.emit("dropRequest", req, socket);
        res.writeHead(503);
        res.end();
      } else if (req.headers.expect !== undefined) {
        handled = true;

        if (continueExpression.test(req.headers.expect)) {
          res._expect_continue = true;
          if (server.listenerCount("checkContinue") > 0) {
            server.emit("checkContinue", req, res);
          } else {
            res.writeContinue();
            server.emit("request", req, res);
          }
        } else if (server.listenerCount("checkExpectation") > 0) {
          server.emit("checkExpectation", req, res);
        } else {
          res.writeHead(417);
          res.end();
        }
      }
    }

    if (!handled) {
      server.emit("request", req, res);
    }
  } finally {
    exitAsyncResourceIfActive(prevAsyncResource);
  }

  return 0;
}

function isRequestKnownEmpty(req) {
  return req.headers["content-length"] === undefined &&
    req.headers["transfer-encoding"] === undefined;
}

// Emit an `http` PerformanceEntry for a finished server request/response, when
// a PerformanceObserver is watching the "http" type. Shared by the classic
// resOnFinish and the native fast path (makeNativeOnRequest), both of which set
// req[kPerfStartTime] when the request starts.
function emitServerHttpPerfEntry(req, res) {
  const perfStartTime = req[kPerfStartTime];
  if (perfStartTime === undefined || !hasNodeObserverForType("http")) {
    return;
  }
  enqueueNodePerformanceEntry({
    name: "HttpRequest",
    entryType: "http",
    startTime: perfStartTime,
    duration: performance.now() - perfStartTime,
    detail: {
      req: {
        method: req.method,
        url: req.url || "/",
        headers: req.headers,
      },
      res: {
        statusCode: res.statusCode,
        statusMessage: res.statusMessage ||
          STATUS_CODES[res.statusCode] || "",
        headers: res.getHeaders(),
      },
    },
  });
}

function resOnFinish(req, res, socket, state, server) {
  if (onServerResponseFinishChannel.hasSubscribers) {
    onServerResponseFinishChannel.publish({
      request: req,
      response: res,
      socket,
      server,
    });
  }

  assert(state.incoming.length === 0 || state.incoming[0] === req);

  // End OTel server span
  const span = res[kOtelSpan];
  if (span) {
    span.setAttribute("http.response.status_code", String(res.statusCode));
    if (res.statusCode >= 500) {
      span.setAttribute("error.type", String(res.statusCode));
      span.setStatus({ code: 2 });
    }
    span.end();
    res[kOtelSpan] = null;
  }

  emitServerHttpPerfEntry(req, res);

  // Record OTel server metrics
  if (res[kOtelStartTime] !== undefined) {
    const durationS = (performance.now() - res[kOtelStartTime]) / 1000;
    const scheme = socket.encrypted ? "https" : "http";
    const metricAttrs = {
      "http.request.method": req.method,
      "http.response.status_code": res.statusCode,
      "network.protocol.version":
        `${req.httpVersionMajor}.${req.httpVersionMinor}`,
      "url.scheme": scheme,
    };
    const metrics = getOtelMetrics();
    metrics.activeRequests.add(-1, {
      "http.request.method": req.method,
      "url.scheme": scheme,
    });
    metrics.requestDuration.record(durationS, metricAttrs);
    metrics.requestBodySize.record(res[kOtelReqBodySize] || 0, metricAttrs);
    metrics.responseBodySize.record(0, metricAttrs);
    res[kOtelStartTime] = undefined;
  }

  ArrayPrototypeShift(state.incoming);

  if (!req._consuming && !req._readableState?.resumeScheduled) {
    req._dump();
  }

  res.detachSocket(socket);
  clearIncoming(req);
  nextTick(emitCloseNT, res);

  // The request is done; stop tracking it for headersTimeout/requestTimeout.
  // Only pop if the entry still belongs to this completed request: when
  // requests are pipelined, the parser has already pushed a fresh entry
  // for the next request via kOnMessageBegin.
  const connections = server[kConnectionsKey];
  if (connections) connections.popActiveIfReq(socket, req);

  if (res._last) {
    if (typeof socket.destroySoon === "function") {
      socket.destroySoon();
    } else {
      socket.end();
    }
  } else if (state.outgoing.length === 0) {
    // If the server is closing, destroy the socket instead of
    // setting a keep-alive timeout (prevents timer leaks).
    if (!server.listening) {
      socket.destroy();
    } else {
      const keepAliveTimeout = NumberIsFinite(server.keepAliveTimeout) &&
          server.keepAliveTimeout >= 0
        ? server.keepAliveTimeout
        : 0;

      if (keepAliveTimeout) {
        const timeoutMsecs = keepAliveTimeout + 1000;
        const suspendedTimeout = state.keepAliveTimeout;
        if (
          state.keepAliveTimeoutSuspended &&
          socket.setTimeout === net.Socket.prototype.setTimeout &&
          socket[kTimeout] === null &&
          state.keepAliveTimeoutMsecs === timeoutMsecs
        ) {
          socket[kTimeout] = suspendedTimeout;
          socket.timeout = timeoutMsecs;
          suspendedTimeout.refresh();
        } else {
          if (state.keepAliveTimeoutSuspended) {
            suspendedTimeout[kDestroy]();
          }
          socket.setTimeout(timeoutMsecs);
          state.keepAliveTimeout = socket[kTimeout];
          state.keepAliveTimeoutMsecs = timeoutMsecs;
        }
        state.keepAliveTimeoutSet = true;
        state.keepAliveTimeoutSuspended = false;
      } else if (
        state.keepAliveTimeoutSuspended
      ) {
        state.keepAliveTimeout[kDestroy]();
        state.keepAliveTimeout = null;
        state.keepAliveTimeoutMsecs = 0;
        state.keepAliveTimeoutSuspended = false;
      }
    }
  } else {
    const m = ArrayPrototypeShift(state.outgoing);
    if (m) {
      m.assignSocket(socket);
    }
  }
}

function clearIncoming(req) {
  req ||= this;
  const parser = req.socket?.parser;
  // Reset the .incoming property so that the request object can be gc'ed.
  if (parser && parser.incoming === req) {
    if (req.readableEnded) {
      parser.incoming = null;
    } else {
      req.on("end", clearIncoming);
    }
  }
}

// ===========================================================================
// Native fast path for node:http servers.
//
// Eligible plain-TCP servers bind through the same native HTTP engine that
// backs Deno.serve (deno_http_h1), dispatching each parsed request straight to
// lightweight IncomingMessage/ServerResponse shims that read/write via the
// op_http_* ops. This bypasses the net.Socket + llhttp + node-stream machinery
// (and its per-request event-loop ticks / async-read parking) on the hot path.
// Anything the shims don't support falls back to the classic polyfill path.
// ===========================================================================

const { listen: denoListen } = core.loadExtScript("ext:deno_net/01_net.js");

const kNativeServeHandle = Symbol("kNativeServeHandle");

// Escape hatch while the fast path matures. Default on; opt out with
// DENO_NODE_HTTP_NATIVE=0. Read without a permission check (no --allow-env
// needed) so the opt-out always works.
let nativeHttpForceDisabled = false;
let nativeHttpEnvChecked = false;
function nativeHttpEnabled() {
  if (!nativeHttpEnvChecked) {
    nativeHttpEnvChecked = true;
    const v = op_get_env_no_permission_check("DENO_NODE_HTTP_NATIVE");
    if (v === "0" || v === "false") {
      nativeHttpForceDisabled = true;
    }
  }
  return !nativeHttpForceDisabled;
}
internals.__disableNodeHttpNative = () => {
  nativeHttpForceDisabled = true;
  nativeHttpEnvChecked = true;
};

// Request-body pump state for a native-mode IncomingMessage (see
// nativeIncomingRead). Request-reading ops borrow (do not consume) the external;
// the response op consumes it.
const BODY_NOT_STARTED = 0;
const BODY_STREAMING = 1;
const BODY_DONE = 2;
const RID_NONE = 4294967295; // ResourceId::MAX

// Decide whether `server` can use the native fast path. Conservative: only
// plain http.Server with the default request/response classes and no listeners
// that need raw socket / upgrade semantics.
// True in a cluster worker (flag set by node:cluster's worker init). Cheap: no
// node:cluster import in the common non-cluster case.
function isClusterWorker() {
  return internals.nodeClusterIsWorker === true;
}

function nativeFastPathEligible(server) {
  if (!nativeHttpEnabled()) return false;
  // Cluster workers virtualize listen() through cluster._getServer (shared-fd /
  // round-robin handoff from the primary); the native fast path binds its own
  // listener and would bypass that, hanging the worker. Fall back to the classic
  // net.Server path (which routes through cluster) for cluster workers.
  if (isClusterWorker()) return false;
  if (server[kServerResponse] !== ServerResponse) return false;
  if (server[kIncomingMessage] !== IncomingMessage) return false;
  if (server.listenerCount("upgrade") > 0) return false;
  if (server.listenerCount("connect") > 0) return false;
  // A user-supplied shouldUpgradeCallback routes requests to the upgrade path;
  // the native path can't honor it, so fall back to the classic path.
  if (server._hasUserShouldUpgradeCallback) return false;
  if (server.listenerCount("clientError") > 0) return false;
  // 'connection' is always present (internal connectionListener); a user
  // listener means they want raw socket access -> fall back.
  if (server.listenerCount("connection") > 1) return false;
  // A server with no 'request' handler isn't dispatching requests (e.g. tests
  // that only exercise raw 'connection'/socket behavior). The native fast path
  // exists to dispatch requests, so there's nothing to gain and it would
  // suppress connection-level events; let the classic net.Server path handle it.
  if (server.listenerCount("request") === 0) return false;
  if (otelState.TRACING_ENABLED || otelState.METRICS_ENABLED) return false;
  // Connection/socket semantics the native fast path can't honor: a per-socket
  // inactivity timeout (server.setTimeout adds a 'timeout' listener and sets
  // server.timeout) needs a real per-connection socket to fire 'timeout' on,
  // and maxRequestsPerSocket needs per-connection request accounting. The
  // classic path handles both, so fall back rather than silently dropping them.
  if (server.timeout > 0 || server.listenerCount("timeout") > 0) return false;
  if (server.maxRequestsPerSocket > 0) return false;
  // Expect: 100-continue handling (checkContinue/checkExpectation) lets the
  // handler decide whether to continue or reject; the native path can't emit
  // those events, so fall back to the classic path.
  if (
    server.listenerCount("checkContinue") > 0 ||
    server.listenerCount("checkExpectation") > 0
  ) {
    return false;
  }
  return true;
}

// Parse listen() args into { port, host } for a plain TCP listen, or null if
// the form isn't an eligible TCP bind (unix path, fd, handle, etc.).
function parseTcpListenArgs(args) {
  let cb;
  if (typeof args[args.length - 1] === "function") {
    cb = args[args.length - 1];
    args = ArrayPrototypeSlice(args, 0, -1);
  }
  const first = args[0];
  if (typeof first === "number" || typeof first === "string") {
    // listen(port[, host])
    const port = Number(first);
    if (!NumberIsFinite(port)) return null;
    const host = typeof args[1] === "string" ? args[1] : undefined;
    return { port, host, cb };
  }
  if (first !== null && typeof first === "object") {
    if (first.path !== undefined || first.fd !== undefined) return null;
    if (first.port === undefined) return null;
    const port = Number(first.port);
    if (!NumberIsFinite(port)) return null;
    return { port, host: first.host, cb, reusePort: first.reusePort };
  }
  return null;
}

// `push` here is Readable.prototype.push, not Array.prototype.push, but the
// prefer-primordials lint can't distinguish them; wrap it once.
// deno-lint-ignore prefer-primordials
const nativePush = (readable, chunk) => readable.push(chunk);

// Pull the request body lazily into the (real) IncomingMessage Readable. The
// common small-body keep-alive case is fully prebuffered by deno_http_h1 and
// delivered in one synchronous chunk; larger bodies stream via a resource.
// Assigned per-instance as `req._read` (the real IncomingMessage._read expects
// a socket, which native mode doesn't have).
function nativeIncomingRead(_n) {
  if (this._nativeBodyState === BODY_DONE) {
    return;
  }
  // A streaming read is already in flight; its continuation self-pumps the
  // next read (see below), so ignore the stream machinery's extra _read calls
  // to avoid issuing concurrent reads on the same body resource.
  if (this._nativeReadInFlight) {
    return;
  }
  if (this._nativeBodyState === BODY_NOT_STARTED) {
    const ext = this[kNativeExternal];
    if (!ext) {
      this._nativeBodyState = BODY_DONE;
      this.complete = true;
      nativePush(this, null);
      return;
    }
    const buffered = op_http_try_take_full_request_body(ext);
    if (buffered !== null) {
      this._nativeBodyState = BODY_DONE;
      this.complete = true;
      nativePush(
        this,
        Buffer.from(
          TypedArrayPrototypeGetBuffer(buffered),
          TypedArrayPrototypeGetByteOffset(buffered),
          TypedArrayPrototypeGetByteLength(buffered),
        ),
      );
      nativePush(this, null);
      return;
    }
    const rid = op_http_read_request_body(ext);
    if (rid === RID_NONE) {
      this._nativeBodyState = BODY_DONE;
      this.complete = true;
      nativePush(this, null);
      return;
    }
    this._nativeStreamRid = rid;
    this._nativeBodyState = BODY_STREAMING;
  }
  const rid = this._nativeStreamRid;
  const buf = new Uint8Array(65536);
  // deno-lint-ignore no-this-alias
  const self = this;
  self._nativeReadInFlight = true;
  PromisePrototypeThen(
    core.read(rid, buf),
    (n) => {
      self._nativeReadInFlight = false;
      if (n === 0) {
        self._nativeBodyState = BODY_DONE;
        self.complete = true;
        core.tryClose(rid);
        nativePush(self, null);
        return;
      }
      // Push the chunk. `push()` returns true while the consumer still wants
      // data; when it does, self-pump the next read directly rather than
      // relying on the Readable's `maybeReadMore` (which does not re-arm an
      // async source reliably). On backpressure (false) we stop and wait for
      // the stream to call `_read` again on drain.
      const more = nativePush(
        self,
        Buffer.from(TypedArrayPrototypeGetBuffer(buf), 0, n),
      );
      if (more && self._nativeBodyState === BODY_STREAMING) {
        FunctionPrototypeCall(nativeIncomingRead, self, 0);
      }
    },
    (_err) => {
      // A body-read failure means the connection went away mid-request (the
      // resource is the connection). Treat it as a client abort, matching the
      // abort watcher in makeNativeOnRequest; gate on `destroyed` so the two
      // paths don't double-destroy with competing errors.
      self._nativeReadInFlight = false;
      core.tryClose(rid);
      if (!self.destroyed) {
        self.destroy(connResetException("aborted"));
      }
    },
  );
}

// Response finished without the handler consuming the request body: discard it
// like Node's `req._dump()`. The engine drains the unread wire body; we just
// remove any 'data' listeners (so a deferred `_read` can't deliver the body),
// mark the body done, and queue EOF -- WITHOUT touching the external (the
// response commit may have freed it). Only acts on a not-yet-started body, so a
// handler that is actively reading (streaming) or already finished is untouched.
function nativeDiscardBody() {
  if (this._nativeBodyState !== BODY_NOT_STARTED) {
    return;
  }
  this._nativeBodyState = BODY_DONE;
  this._dumped = true;
  this.complete = true;
  this.removeAllListeners("data");
  nativePush(this, null);
}

// Strip the origin from an absolute URL to get Node's req.url (request target).
function nativeRequestTarget(full) {
  const schemeEnd = StringPrototypeIndexOf(full, "://");
  if (schemeEnd === -1) return full;
  const pathStart = StringPrototypeIndexOf(full, "/", schemeEnd + 3);
  return pathStart === -1 ? "/" : StringPrototypeSlice(full, pathStart);
}

// Build a real IncomingMessage backed by the native request `external` so that
// frameworks which re-parent onto http.IncomingMessage.prototype keep working.
// Synthetic socket exposed as `req.socket` / `res.socket` on the native path.
// The deno_http_h1 engine owns the real TCP connection, so this is a stand-in
// that provides the net.Socket surface node:http handlers and middleware read
// (remote/local address, byte counters, timeouts, the Writable stream methods).
// It is NOT wired to the real connection: writes are discarded and destroy()
// does not close the underlying connection (the response ops drive that).
class NativeFakeSocket extends Duplex {
  constructor() {
    super({ autoDestroy: false });
    this.remoteAddress = undefined;
    this.remotePort = undefined;
    this.remoteFamily = undefined;
    this.localAddress = undefined;
    this.localPort = undefined;
    this.encrypted = undefined;
    this.bytesRead = 0;
    this.bytesWritten = 0;
    this.connecting = false;
    this._httpMessage = null;
    this.parser = null;
    this.timeout = 0;
    // Real net.Sockets initialize this to null; some code (and tests, e.g.
    // test-child-process-http-socket-leak) reads `socket[kTimeout]`.
    this[kTimeout] = null;
    this._handle = { __proto__: null, writeQueueSize: 0, reading: false };
    // Set when the response is demoted to classic mode (the handler took over
    // raw socket writes, e.g. test-http-response-cork): a real net.Socket on the
    // reclaimed fd that writes reach the wire through. See demoteNativeResponse.
    this._realBacking = null;
  }

  _read() {}

  _write(chunk, encoding, callback) {
    if (this._realBacking !== null && this._realBacking !== undefined) {
      // Queue to the real socket and signal this write complete immediately, so
      // the synthetic socket's (corked) write machinery flushes all chunks in
      // one tick. Threading the real socket's async completion back here would
      // stall the flush after the first chunk (it never advances). The real
      // socket buffers and writes the queued chunks to the wire in order.
      this._realBacking.write(chunk, encoding);
      callback();
      return;
    }
    this.bytesWritten += chunk.length;
    callback();
  }

  _final(callback) {
    callback();
  }

  // The engine owns the real connection, so destroying the synthetic socket
  // tears down the in-flight response (which drives the connection): a handler
  // that calls `res.socket.destroy()` mid-response aborts it like Node does
  // (e.g. test-http-client-spurious-aborted truncates a Content-Length body).
  _destroy(err, callback) {
    if (this._timeoutTimer !== undefined) {
      clearTimeout(this._timeoutTimer);
      this._timeoutTimer = undefined;
    }
    const res = this._httpMessage;
    if (res !== undefined && res !== null && !res.destroyed) {
      res.destroy(err || undefined);
    }
    callback(err);
  }

  setTimeout(msecs, callback) {
    this.timeout = msecs;
    if (callback !== undefined) {
      if (msecs === 0) {
        this.removeListener("timeout", callback);
      } else {
        this.once("timeout", callback);
      }
    }
    // The engine owns the real I/O, so schedule a coarse one-shot inactivity
    // timer (cleared on destroy): it fires 'timeout' if the connection is still
    // alive after `msecs`, enough for a handler that stalls (test-http-set-timeout).
    if (this._timeoutTimer !== undefined) {
      clearTimeout(this._timeoutTimer);
      this._timeoutTimer = undefined;
    }
    if (msecs > 0) {
      this._timeoutTimer = setTimeout(() => {
        this._timeoutTimer = undefined;
        this.emit("timeout");
      }, msecs);
      if (typeof this._timeoutTimer?.unref === "function") {
        this._timeoutTimer.unref();
      }
    }
    return this;
  }

  _onTimeout() {}

  setKeepAlive() {
    return this;
  }

  setNoDelay() {
    return this;
  }

  ref() {
    return this;
  }

  unref() {
    return this;
  }

  address() {
    return {
      address: this.localAddress,
      port: this.localPort,
      family: this.remoteFamily ?? "IPv4",
    };
  }

  // Reclaim the real OS file descriptor behind this synthetic socket from the
  // HTTP engine so it can be exposed as a real socket (e.g. handed to a child
  // process over IPC via child.send(msg, res.socket)). Only works while the
  // request is being dispatched synchronously (the bodiless path). Returns the
  // fd, or -1 if it can't be reclaimed. On success the native response is
  // abandoned: the engine relinquishes the connection, so the caller (or
  // whoever receives the fd) must drive the response. See op_http_reclaim_socket.
  _nativeReclaimFd() {
    const res = this._httpMessage;
    const external = res != null ? res[kNativeExternal] : undefined;
    if (external === null || external === undefined) {
      return -1;
    }
    const fd = op_http_reclaim_socket(external);
    if (fd < 0) {
      return -1;
    }
    // The external was consumed by the op; stop driving the native response.
    res[kNativeExternal] = null;
    if (res.req != null && res.req[kNativeExternal] !== undefined) {
      res.req[kNativeExternal] = null;
    }
    return fd;
  }
}

// Make `req.socket`/`res.socket` pass `instanceof net.Socket` (some handlers
// assert it, e.g. test-http-set-timeout). The fake can't extend net.Socket --
// its getter-only `remoteAddress`/etc. conflict with the fake's data props -- so
// brand it via Symbol.hasInstance (additive: real net.Sockets still match).
const realSocketHasInstance = net.Socket[SymbolHasInstance];
ObjectDefineProperty(net.Socket, SymbolHasInstance, {
  __proto__: null,
  configurable: true,
  writable: true,
  value(obj) {
    return ObjectPrototypeIsPrototypeOf(NativeFakeSocket.prototype, obj) ||
      FunctionPrototypeCall(realSocketHasInstance, this, obj);
  },
});

// Build the synthetic socket for a native request: remote address from the
// engine, local address from the listener.
function makeNativeSocket(external, server) {
  const socket = new NativeFakeSocket();
  try {
    const addr = op_http_get_request_remote_addr(external);
    if (
      addr && addr[0] !== "unix" &&
      !StringPrototypeStartsWith(addr[0], "vsock:")
    ) {
      socket.remoteAddress = addr[0];
      socket.remotePort = addr[1];
      socket.remoteFamily = StringPrototypeIncludes(addr[0], ":")
        ? "IPv6"
        : "IPv4";
    }
  } catch {
    // External already consumed; leave the remote address undefined.
  }
  const handle = server[kNativeServeHandle];
  if (handle !== undefined && handle.addr !== undefined) {
    socket.localAddress = handle.addr.hostname;
    socket.localPort = handle.addr.port;
  }
  // Node attaches an error handler to every server connection socket; without
  // one a `res.socket.emit('error', ...)` (e.g. test-http-header-badrequest)
  // throws as an unhandled 'error'. Route it to 'clientError' if observed, else
  // tear down the in-flight response (or the socket) like Node's default does.
  socket.on("error", (err) => {
    if (server.listenerCount("clientError") > 0) {
      server.emit("clientError", err, socket);
      return;
    }
    const res = socket._httpMessage;
    if (res !== undefined && res !== null && !res.destroyed) {
      res.destroy(err);
    } else if (!socket.destroyed) {
      socket.destroy(err);
    }
  });
  // A handler that manually emits 'close' on its socket uses Node's freeParser
  // pattern to abort any pipelined requests still in the buffer (e.g.
  // test-http-parser-freed-during-execute). The native engine reads pipelined
  // requests response-ordered, so force the in-flight response to non-keepalive:
  // the serve loop then closes the connection after it instead of dispatching
  // the next pipelined request. The normal end-of-connection 'close' fires after
  // the response committed, so setting this is a no-op there.
  socket.on("close", () => {
    const res = socket._httpMessage;
    if (res !== undefined && res !== null && !res.destroyed) {
      res.shouldKeepAlive = false;
      // The serve loop derives keep-alive from the request, so flag the response
      // to force the connection closed once it commits (nativeCommit honors it).
      res._nativeForceClose = true;
    }
  });
  // node:http attaches an llhttp parser to every connection socket and frees it
  // when the connection closes (freeParser in _http_common.js). The native
  // engine parses in Rust, but some code reads/overrides `socket.parser.free` /
  // `.close` (e.g. to keep the parser in server.connectionList), so expose a
  // minimal stand-in; `free()` is invoked on connection close (see isClose).
  socket.parser = {
    free() {},
    close() {},
  };
  return socket;
}

function createNativeIncomingMessage(
  external,
  socket,
  maxHeaderPairs,
  joinDuplicateHeaders,
) {
  const req = new IncomingMessage(null);
  req.socket = socket;
  req[kNativeExternal] = external;
  req._nativeBodyState = BODY_NOT_STARTED;
  req._nativeStreamRid = RID_NONE;
  req._nativeReadInFlight = false;
  req._nativeDiscardBody = nativeDiscardBody;
  // `_addHeaderLines` below reads this to decide whether to comma-join
  // duplicate headers (e.g. authorization) vs keep-first; set it before.
  if (joinDuplicateHeaders) {
    req.joinDuplicateHeaders = true;
  }
  const minor = op_http_get_request_http_minor_version(external);
  req.httpVersionMajor = 1;
  req.httpVersionMinor = minor;
  req.httpVersion = minor === 0 ? "1.0" : "1.1";
  req.method = op_http_get_request_method(external);
  req.url = nativeRequestTarget(op_http_get_request_url(external));
  // `server.maxHeadersCount` limits how many request headers are parsed (the
  // classic path caps llhttp at `maxHeadersCount << 1` pairs). The flat array
  // holds 2 entries (name, value) per header, so truncate to that many entries.
  let flat = op_http_get_request_headers(external);
  if (maxHeaderPairs > 0 && flat.length > maxHeaderPairs) {
    flat = ArrayPrototypeSlice(flat, 0, maxHeaderPairs);
  }
  req._addHeaderLines(flat, flat.length);
  req._read = nativeIncomingRead;
  // No request body (no Content-Length / Transfer-Encoding): mark it complete
  // and queue EOF so the stream can 'end' (and autoDestroy -> 'close') when
  // drained, without ever touching the external (which the response commit may
  // already have freed).
  const h = req.headers;
  if (
    h["content-length"] === undefined && h["transfer-encoding"] === undefined
  ) {
    req._nativeBodyState = BODY_DONE;
    req.complete = true;
    nativePush(req, null);
  }
  return req;
}

// The handler took over raw socket writes (assigned `res.socket.write`) before
// the response committed -- e.g. test-http-response-cork, which intercepts the
// socket's writes and corks the socket through the response. The native engine
// writes the response via ops, so those writes never reach `res.socket`. Reclaim
// the real connection fd, back the synthetic socket with a real net.Socket, and
// clear native mode so writeHead/write/end run the classic OutgoingMessage path
// (writing through res.socket exactly like Node, preserving its write framing).
function demoteNativeResponse(res) {
  const socket = res.socket;
  if (
    socket === null || socket === undefined ||
    typeof socket._nativeReclaimFd !== "function"
  ) {
    return;
  }
  // Reclaims the fd and consumes the external (nulls res[kNativeExternal] and
  // res.req[kNativeExternal]); returns the fd or -1 if it can't be reclaimed.
  const fd = socket._nativeReclaimFd();
  if (fd < 0) {
    return;
  }
  socket._realBacking = new net.Socket({
    fd,
    readable: true,
    writable: true,
  });
}

function makeNativeOnRequest(server) {
  // connId -> the one synthetic socket shared by every request on that H1
  // connection (Node gives all keep-alive requests the same `req.socket`). The
  // engine calls us with `(external, connId, isClose)`; on close we drop and
  // destroy the socket. connId is unique per connection (engine thread-local).
  const sockets = new SafeMap();
  return (external, connId, isClose, isTimeout) => {
    if (isTimeout) {
      // node:http server.keepAliveTimeout fired on an idle connection. Mirror
      // the classic socketOnTimeout: emit 'timeout' on the last response and the
      // server; if nothing handles it, destroy the socket. The serve loop closes
      // the connection afterward regardless.
      const socket = MapPrototypeGet(sockets, connId);
      if (socket !== undefined && !socket.destroyed) {
        const serverTimeout = server.emit("timeout", socket);
        if (!serverTimeout) {
          socket.destroy();
        }
      }
      return;
    }
    if (isClose) {
      const socket = MapPrototypeGet(sockets, connId);
      if (socket !== undefined) {
        MapPrototypeDelete(sockets, connId);
        // Free the parser stand-in like Node's freeParser does on close (some
        // handlers override socket.parser.free and expect it to run).
        const parser = socket.parser;
        if (parser !== undefined && parser !== null) {
          socket.parser = null;
          parser.free();
        }
        // Emits 'close' on the socket (Duplex emitClose default). The current
        // req/res 'close' is handled on response finish.
        socket.destroy();
      }
      return;
    }
    let socket = MapPrototypeGet(sockets, connId);
    if (socket === undefined) {
      socket = makeNativeSocket(external, server);
      MapPrototypeSet(sockets, connId, socket);
    }
    // Whether a PRIOR pipelined handler already destroyed this shared, per-
    // connection socket. Captured before this handler runs: socket.destroy() on
    // an already-destroyed Duplex is a no-op, so a request arriving on such a
    // socket can never have its response aborted through it. Using the pre-
    // dispatch value avoids tripping on a socket torn down later by normal
    // completion mid-pipeline (which must NOT abort a live response).
    const socketDestroyedBeforeDispatch = socket.destroyed;
    const maxHeaderPairs = typeof server.maxHeadersCount === "number" &&
        server.maxHeadersCount > 0
      ? server.maxHeadersCount << 1
      : 0;
    const req = createNativeIncomingMessage(
      external,
      socket,
      maxHeaderPairs,
      server.joinDuplicateHeaders === true,
    );
    if (server.highWaterMark !== undefined) {
      req._readableState.highWaterMark = server.highWaterMark;
    }
    // A chunked request can carry trailers after the body. The engine parsed
    // them onto the record before dispatch; fetch them now (while the external
    // is valid -- a later response commit may free it) and apply them on the
    // request's 'end', before the user's 'end' handler runs, so req.rawTrailers
    // / req.trailers are populated regardless of how the body is consumed.
    if (req.headers["transfer-encoding"] !== undefined) {
      const trailers = op_http_get_request_trailers(external);
      if (trailers.length > 0) {
        req.prependOnceListener("end", () => {
          req._addHeaderLines(trailers, trailers.length);
        });
      }
    }
    // optimizeEmptyRequests: a request with no body (no Content-Length /
    // Transfer-Encoding) is dumped and its readable closed up front, so the
    // handler sees `req._dumped`/readableEnded/destroyed (matches the classic
    // path) instead of an empty body to drain.
    if (server.optimizeEmptyRequests && isRequestKnownEmpty(req)) {
      req._dumpAndCloseReadable();
    }
    // Real ServerResponse in native mode: writeHead/write/end branch to the
    // op_http_* ops (see _http_outgoing.ts). Because it IS an http.ServerResponse,
    // frameworks like Express that re-parent `res` keep working.
    const res = new server[kServerResponse](req, {
      __proto__: null,
      rejectNonStandardBodyWrites: server.rejectNonStandardBodyWrites,
      highWaterMark: server.highWaterMark,
    });
    // Node keep-alive intent from the request: HTTP/1.1 keeps alive unless
    // `Connection: close`; HTTP/1.0 only with `Connection: keep-alive`. Drives
    // the JS-side `Connection: keep-alive` response header (nativeWireHeaders).
    const connHeader = req.headers.connection;
    const connLower = typeof connHeader === "string"
      ? StringPrototypeToLowerCase(connHeader)
      : "";
    res.shouldKeepAlive = req.httpVersionMinor === 1
      ? connLower !== "close"
      : connLower === "keep-alive";
    res._keepAliveTimeout = server.keepAliveTimeout;
    res[kNativeExternal] = external;
    res.socket = socket;
    socket._httpMessage = res;
    // httpAllowHalfOpen: a client half-close (FIN) must not cancel an in-flight
    // response. The option is read in JS (set any time, incl. after listen), so
    // tell the engine per request not to abort this response on peer close.
    if (server.httpAllowHalfOpen) {
      op_http_set_allow_half_open(external);
    }
    // maxRequestsPerSocket: count requests on this connection; once the limit is
    // exceeded, drop the request (emit 'dropRequest' if observed, else 503) and
    // don't run the handler. Mirrors parserOnIncoming. (Eligibility falls back to
    // the classic path only when the limit is set before listen; set after, the
    // server takes the native path and this enforces it.)
    const maxRequestsPerSocket = server.maxRequestsPerSocket;
    if (
      typeof maxRequestsPerSocket === "number" && maxRequestsPerSocket > 0
    ) {
      socket._requestsCount = (socket._requestsCount || 0) + 1;
      res.maxRequestsOnConnectionReached =
        maxRequestsPerSocket <= socket._requestsCount;
      if (maxRequestsPerSocket < socket._requestsCount) {
        server.emit("dropRequest", req, socket);
        res.writeHead(503);
        res.end();
        return;
      }
    }
    // perf_hooks `http` instrumentation: when a PerformanceObserver watches the
    // "http" type, stamp the start time and emit an HttpRequest entry on finish
    // (the classic path does this in parserOnIncoming/resOnFinish).
    if (hasNodeObserverForType("http")) {
      req[kPerfStartTime] = performance.now();
      res.once("finish", () => emitServerHttpPerfEntry(req, res));
    }
    // RFC 7230 5.4: an HTTP/1.1 request without a Host header is a 400 (the
    // handler must not run). Mirrors the classic path; gated on requireHostHeader.
    if (
      req.httpVersionMajor === 1 && req.httpVersionMinor === 1 &&
      server.requireHostHeader !== false && req.headers.host === undefined
    ) {
      res.writeHead(400, ["Connection", "close"]);
      res.end();
      return;
    }
    // Expect header (RFC 7231 5.1.1): an HTTP/1.1 request with an Expect value
    // other than 100-continue must not run the request handler -- emit
    // `checkExpectation` if anyone listens, else auto-417 Expectation Failed.
    // (100-continue is handled by the engine / checkContinue eligibility.)
    const expectHeader = req.headers.expect;
    if (
      expectHeader !== undefined && req.httpVersionMajor === 1 &&
      req.httpVersionMinor === 1 &&
      !StringPrototypeIncludes(
        StringPrototypeToLowerCase(expectHeader),
        "100-continue",
      )
    ) {
      if (server.listenerCount("checkExpectation") > 0) {
        server.emit("checkExpectation", req, res);
      } else {
        res.writeHead(417);
        res.end();
      }
      return;
    }
    try {
      // node:http runs each request handler inside its own async resource so
      // async_hooks / executionAsyncResource() observe per-request context (the
      // classic path runs in the IncomingMessage's async resource). Gated on
      // active hooks so the no-hook fast path stays allocation-free.
      if (enabledHooksExist()) {
        new AsyncResource("HTTPINCOMINGMESSAGE").runInAsyncScope(() => {
          server.emit("request", req, res);
        });
      } else {
        server.emit("request", req, res);
      }
    } catch (err) {
      // Always finish the response so the connection can't hang. If headers
      // weren't sent yet we can still turn it into a 500; if they were (e.g. the
      // handler threw after writeHead), just end it so the request completes.
      try {
        if (!res.headersSent) {
          res.statusCode = 500;
        }
        if (!res.finished) {
          res.end();
        }
      } catch { /* external already consumed */ }
      internals.log("error", "Error in node:http request handler", err);
    }
    // A prior pipelined handler may have destroyed the (shared, per-connection)
    // socket. Since socket.destroy() on an already-destroyed Duplex is a no-op,
    // this handler's res can never be aborted through the socket and would
    // otherwise leave the response-ordered serve loop waiting forever for a
    // reply that never comes. The handler has already run (mustCall satisfied),
    // so abort the uncommitted response and let the loop move on (Node discards
    // responses on a destroyed socket too).
    if (
      socketDestroyedBeforeDispatch && !res.finished &&
      res[kNativeExternal] !== null && res[kNativeExternal] !== undefined
    ) {
      op_http_abort_response(external);
      return;
    }
    // If the handler returned without committing a response, watch for a client
    // abort. The hot path (sync res.end) already consumed the external, so this
    // op is only armed for still-open (async/streaming/no-reply) responses.
    if (res[kNativeExternal] !== null && res[kNativeExternal] !== undefined) {
      PromisePrototypeThen(
        op_http_request_on_cancel(external),
        (cancelled) => {
          if (!cancelled || req.destroyed) {
            return;
          }
          // With lifecycle listeners, propagate the abort as Node does:
          // req.destroy(aborted) -> 'aborted'/'error'/'close'.
          if (
            req.listenerCount("aborted") > 0 ||
            req.listenerCount("close") > 0 ||
            req.listenerCount("error") > 0 || res.listenerCount("close") > 0
          ) {
            req.destroy(connResetException("aborted"));
            return;
          }
          // No lifecycle listeners (e.g. a handler that never replies while the
          // client times out): end the response so the serve loop and
          // server.close() can complete, without surfacing an unhandled 'error'.
          const ext = res[kNativeExternal];
          if (ext !== null && ext !== undefined) {
            op_http_abort_response(ext);
          }
        },
      );
    }
  };
}

// Try to bind `server` via the native fast path. Returns true if it took over
// the listen, false to fall back to the classic net.Server path.
function tryListenNative(server, args) {
  if (!nativeFastPathEligible(server)) return false;
  const parsed = parseTcpListenArgs(args);
  if (parsed === null) return false;

  let listener;
  try {
    listener = denoListen({
      hostname: parsed.host ?? "0.0.0.0",
      port: parsed.port,
      transport: "tcp",
      reusePort: parsed.reusePort ?? false,
    });
  } catch {
    // Couldn't bind natively (e.g. unsupported option); fall back.
    return false;
  }

  // 00_serve.ts is lazy-loaded; loadExtScript forces its evaluation and
  // returns the serve entry points (the IIFE also wires up internals).
  const { serveHttpOnListenerForNode } = core.loadExtScript(
    "ext:deno_http/00_serve.ts",
  );
  const handle = serveHttpOnListenerForNode(
    listener,
    undefined,
    makeNativeOnRequest(server),
    undefined,
    () => {},
    {
      __proto__: null,
      keepAliveTimeoutMs: server.keepAliveTimeout,
      headersTimeoutMs: server.headersTimeout,
      requestTimeoutMs: server.requestTimeout,
    },
  );
  server[kNativeServeHandle] = handle;
  const addr = handle.addr;

  server._handle = {
    close(cb) {
      PromisePrototypeThen(handle.shutdown(), () => cb && cb());
    },
    ref() {
      handle.ref();
    },
    unref() {
      handle.unref();
    },
  };
  // The classic net.Server listen path applies a pre-listen `unref()` in
  // `_listen2`; the native path bypasses it, so honor `server.unref()` that ran
  // before `listen()` here (otherwise the serve op keeps the loop alive).
  if (server._unref) {
    handle.unref();
  }
  server.address = function address() {
    return {
      address: addr.hostname,
      port: addr.port,
      family: StringPrototypeIncludes(addr.hostname, ":") ? "IPv6" : "IPv4",
    };
  };
  // `server.listening` is a getter derived from `_handle` (set above).
  if (parsed.cb) {
    server.once("listening", parsed.cb);
  }
  nextTick(() => server.emit("listening"));
  return true;
}

function Server(options, requestListener) {
  if (!ObjectPrototypeIsPrototypeOf(Server.prototype, this)) {
    return new Server(options, requestListener);
  }

  if (typeof options === "function") {
    requestListener = options;
    options = kEmptyObject;
  } else if (options == null) {
    options = kEmptyObject;
  } else {
    validateObject(options, "options");
  }

  FunctionPrototypeCall(storeHTTPOptions, this, options);

  FunctionPrototypeCall(net.Server, this, {
    allowHalfOpen: true,
    noDelay: options.noDelay ?? true,
    keepAlive: options.keepAlive,
    keepAliveInitialDelay: options.keepAliveInitialDelay,
    highWaterMark: options.highWaterMark,
  });

  if (requestListener) {
    this.on("request", requestListener);
  }

  this.httpAllowHalfOpen = false;

  this.on("connection", connectionListener);
  this.on("listening", setupConnectionsTracking);

  this.timeout = 0;
  this.maxHeadersCount = null;
  this.maxRequestsPerSocket = 0;

  this[kUniqueHeaders] = parseUniqueHeadersOption(options.uniqueHeaders);
}

// Wraps a deno_core system interval ID so the stored handle exposes a
// Node-compatible `_destroyed` flag. Tests (and Node user code) read
// `server[kConnectionsCheckingInterval]._destroyed` to confirm the
// interval was cleared.
class ConnectionsCheckingInterval {
  _timerId = 0;
  _destroyed = false;
}

function setupConnectionsTracking() {
  this[kConnectionsKey] ||= new ConnectionsList();

  destroyConnectionsCheckingInterval(this[kConnectionsCheckingInterval]);

  const interval = this.connectionsCheckingInterval || 30_000;
  const handle = new ConnectionsCheckingInterval();
  handle._timerId = core.createSystemInterval(
    FunctionPrototypeBind(checkConnections, this),
    interval,
  );
  this[kConnectionsCheckingInterval] = handle;
}

function destroyConnectionsCheckingInterval(handle) {
  if (handle && !handle._destroyed) {
    core.cancelTimer(handle._timerId);
    handle._destroyed = true;
  }
}

function checkConnections() {
  if (this.headersTimeout === 0 && this.requestTimeout === 0) {
    return;
  }

  const expired = this[kConnectionsKey].expired(
    this.headersTimeout,
    this.requestTimeout,
  );

  for (let i = 0; i < expired.length; i++) {
    const socket = expired[i].socket;
    if (socket) {
      onRequestTimeout(socket);
    }
  }
}

function httpServerPreClose(server) {
  server.closeIdleConnections();
  destroyConnectionsCheckingInterval(server[kConnectionsCheckingInterval]);
}

function storeHTTPOptions(options) {
  this[kIncomingMessage] = options.IncomingMessage || IncomingMessage;
  this[kServerResponse] = options.ServerResponse || ServerResponse;

  const highWaterMark = options.highWaterMark;
  if (highWaterMark !== undefined) {
    validateInteger(highWaterMark, "options.highWaterMark", 1);
  }
  this.highWaterMark = highWaterMark;

  const maxHeaderSize = options.maxHeaderSize;
  if (maxHeaderSize !== undefined) {
    validateInteger(maxHeaderSize, "maxHeaderSize", 0);
  }
  this.maxHeaderSize = maxHeaderSize;

  const insecureHTTPParser = options.insecureHTTPParser;
  if (insecureHTTPParser !== undefined) {
    validateBoolean(insecureHTTPParser, "options.insecureHTTPParser");
  }
  this.insecureHTTPParser = insecureHTTPParser;

  const optimizeEmptyRequests = options.optimizeEmptyRequests;
  if (optimizeEmptyRequests !== undefined) {
    validateBoolean(
      optimizeEmptyRequests,
      "options.optimizeEmptyRequests",
    );
  }
  this.optimizeEmptyRequests = optimizeEmptyRequests;

  const requestTimeout = options.requestTimeout;
  if (requestTimeout !== undefined) {
    validateInteger(requestTimeout, "requestTimeout", 0);
    this.requestTimeout = requestTimeout;
  } else {
    this.requestTimeout = 300_000; // 5 minutes
  }

  const headersTimeout = options.headersTimeout;
  if (headersTimeout !== undefined) {
    validateInteger(headersTimeout, "headersTimeout", 0);
    this.headersTimeout = headersTimeout;
  } else {
    this.headersTimeout = MathMin(60_000, this.requestTimeout);
  }

  if (
    this.requestTimeout > 0 && this.headersTimeout > 0 &&
    this.headersTimeout > this.requestTimeout
  ) {
    throw new ERR_OUT_OF_RANGE(
      "headersTimeout",
      "<= requestTimeout",
      headersTimeout,
    );
  }

  const keepAliveTimeout = options.keepAliveTimeout;
  if (keepAliveTimeout !== undefined) {
    validateInteger(keepAliveTimeout, "keepAliveTimeout", 0);
    this.keepAliveTimeout = keepAliveTimeout;
  } else {
    this.keepAliveTimeout = 5_000;
  }

  const connectionsCheckingInterval = options.connectionsCheckingInterval;
  if (connectionsCheckingInterval !== undefined) {
    validateInteger(
      connectionsCheckingInterval,
      "connectionsCheckingInterval",
      1,
    );
    this.connectionsCheckingInterval = connectionsCheckingInterval;
  } else {
    this.connectionsCheckingInterval = 30_000;
  }

  const requireHostHeader = options.requireHostHeader;
  if (requireHostHeader !== undefined) {
    validateBoolean(requireHostHeader, "options.requireHostHeader");
    this.requireHostHeader = requireHostHeader;
  } else {
    this.requireHostHeader = true;
  }

  const joinDuplicateHeaders = options.joinDuplicateHeaders;
  if (joinDuplicateHeaders !== undefined) {
    validateBoolean(
      joinDuplicateHeaders,
      "options.joinDuplicateHeaders",
    );
  }
  this.joinDuplicateHeaders = joinDuplicateHeaders;

  const rejectNonStandardBodyWrites = options.rejectNonStandardBodyWrites;
  if (rejectNonStandardBodyWrites !== undefined) {
    validateBoolean(
      rejectNonStandardBodyWrites,
      "options.rejectNonStandardBodyWrites",
    );
    this.rejectNonStandardBodyWrites = rejectNonStandardBodyWrites;
  } else {
    this.rejectNonStandardBodyWrites = false;
  }

  const shouldUpgradeCallback = options.shouldUpgradeCallback;
  if (shouldUpgradeCallback !== undefined) {
    validateFunction(
      shouldUpgradeCallback,
      "options.shouldUpgradeCallback",
    );
    this.shouldUpgradeCallback = shouldUpgradeCallback;
    // A user-supplied callback can route any request to the upgrade path; the
    // native fast path can't honor it, so mark the server ineligible. (The
    // default below only upgrades when there's an 'upgrade' listener, which
    // nativeFastPathEligible already checks.)
    this._hasUserShouldUpgradeCallback = true;
  } else {
    this.shouldUpgradeCallback = function () {
      return this.listenerCount("upgrade") > 0;
    };
  }
}
ObjectSetPrototypeOf(Server.prototype, net.Server.prototype);
ObjectSetPrototypeOf(Server, net.Server);

// Applies the DENO_SERVE_ADDRESS override before delegating to
// net.Server.prototype.listen.
//
// TCP overrides rewrite the address passed to listen(). Non-TCP and
// duplicate overrides spin up a separate Deno listener that feeds
// synthetic "connection" events into this server.
Server.prototype.listen = function listen(...args) {
  const applied = applyAddressOverride();

  switch (applied.mode) {
    case "none":
      if (tryListenNative(this, args)) {
        return this;
      }
      return FunctionPrototypeApply(net.Server.prototype.listen, this, args);

    case "tcp": {
      // Rewrite the listen args so the normal net.Server code binds
      // to the override address instead of what the app requested.
      let cb;
      const last = args[args.length - 1];
      if (typeof last === "function") {
        cb = last;
        args = ArrayPrototypeSlice(args, 0, -1);
      }
      const rewritten = [{ host: applied.host, port: applied.port }];
      if (cb) ArrayPrototypePush(rewritten, cb);
      this.once("listening", notifyAddressOverrideServing);
      return FunctionPrototypeApply(
        net.Server.prototype.listen,
        this,
        rewritten,
      );
    }

    case "override-only": {
      // Don't bind the app-supplied TCP address at all -- the override
      // listener is the only way into this server. `listening` on
      // net.Server is derived from `_handle`, so install a stub that
      // reports listening without owning a real OS handle.
      let cb;
      const last = args[args.length - 1];
      if (typeof last === "function") cb = last;
      if (cb) this.once("listening", cb);
      this._handle = {
        close() {},
        ref() {},
        unref() {},
      };
      startOverrideListener(this, applied.override, connectionListener);
      nextTick(() => this.emit("listening"));
      return this;
    }

    case "duplicate": {
      startOverrideListener(this, applied.override, connectionListener);
      return FunctionPrototypeApply(net.Server.prototype.listen, this, args);
    }
  }
};

Server.prototype.setTimeout = function setTimeout(msecs, callback) {
  this.timeout = msecs;
  if (callback) {
    this.on("timeout", callback);
  }
  return this;
};

Server.prototype.close = function close(cb) {
  httpServerPreClose(this);
  return FunctionPrototypeCall(net.Server.prototype.close, this, cb);
};

Server.prototype.closeAllConnections = function closeAllConnections() {
  const connections = this[kConnectionsKey];
  if (connections) {
    for (const socket of new SafeSetIterator(connections._all)) {
      socket.destroy();
    }
  }
};

Server.prototype.closeIdleConnections = function closeIdleConnections() {
  const connections = this[kConnectionsKey];
  if (connections) {
    for (const socket of new SafeSetIterator(connections._all)) {
      // A socket is idle if it completed a request-response cycle and
      // currently has no active HTTP response being written. Sockets
      // that have never finished a response (e.g. still receiving
      // headers) are not idle.
      if (!socket._httpMessage && socket._httpMessageDetached) {
        socket.destroy();
      }
    }
  }
};

export {
  applyAddressOverride,
  connectionListener as _connectionListener,
  httpServerPreClose,
  kConnectionsCheckingInterval,
  kIncomingMessage,
  kServerResponse,
  notifyAddressOverrideServing,
  Server,
  ServerResponse,
  setupConnectionsTracking,
  startOverrideListener,
  STATUS_CODES,
  storeHTTPOptions,
};

export default {
  _connectionListener: connectionListener,
  httpServerPreClose,
  kConnectionsCheckingInterval,
  kIncomingMessage,
  kServerResponse,
  Server,
  ServerResponse,
  setupConnectionsTracking,
  STATUS_CODES,
  storeHTTPOptions,
};
