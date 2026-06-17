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

import { core, primordials } from "ext:core/mod.js";
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
  MathMin,
  NumberIsFinite,
  ObjectKeys,
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
  Symbol,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
} = primordials;

import net from "node:net";
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { ok: assert } = core.loadExtScript("ext:deno_node/assert.ts");
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
const { kNeedDrain, kOutHeaders } = core.loadExtScript(
  "ext:deno_node/internal/http.ts",
);
import { IncomingMessage } from "node:_http_incoming";
const {
  connResetException,
  ERR_HTTP_HEADERS_SENT,
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

ServerResponse.prototype.writeContinue = function writeContinue(cb) {
  this._writeRaw("HTTP/1.1 100 Continue\r\n\r\n", "ascii", cb);
  this._sent100 = true;
};

ServerResponse.prototype.writeProcessing = function writeProcessing(cb) {
  this._writeRaw("HTTP/1.1 102 Processing\r\n\r\n", "ascii", cb);
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

  this._writeRaw(head, "ascii", cb);
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

  let headers;
  if (this[kOutHeaders]) {
    // Slow-case: progressive API and header fields are passed.
    if (ArrayIsArray(obj)) {
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

  // Emit HttpRequest perf entry
  const perfStartTime = req[kPerfStartTime];
  if (perfStartTime !== undefined && hasNodeObserverForType("http")) {
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
  connectionListener as _connectionListener,
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
