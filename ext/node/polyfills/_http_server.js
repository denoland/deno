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

// deno-lint-ignore-file prefer-primordials

import { core, primordials } from "ext:core/mod.js";
const {
  ArrayIsArray,
  Error,
  MathMin,
  NumberIsFinite,
  ObjectKeys,
  ObjectSetPrototypeOf,
  Symbol,
} = primordials;

import net from "node:net";
import { Buffer } from "node:buffer";
import { ok as assert } from "node:assert";
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
import { kNeedDrain, kOutHeaders } from "ext:deno_node/internal/http.ts";
import { IncomingMessage } from "node:_http_incoming";
import {
  connResetException,
  ERR_HTTP_HEADERS_SENT,
  ERR_HTTP_SOCKET_ASSIGNED,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_CHAR,
  ERR_OUT_OF_RANGE,
} from "ext:deno_node/internal/errors.ts";
import { kEmptyObject } from "ext:deno_node/internal/util.mjs";
import {
  validateBoolean,
  validateInteger,
  validateLinkHeaderValue,
  validateObject,
} from "ext:deno_node/internal/validators.mjs";
import { nextTick } from "ext:deno_node/_next_tick.ts";
const {
  builtinTracer,
  ContextManager,
  METRICS_ENABLED,
  PROPAGATORS,
  telemetry,
  TRACING_ENABLED,
} = core.loadExtScript("ext:deno_telemetry/telemetry.ts");

const kServerResponse = Symbol("ServerResponse");
const kConnectionsKey = Symbol("http.server.connections");
const kConnectionsCheckingInterval = Symbol(
  "http.server.connectionsCheckingInterval",
);
const kOtelSpan = Symbol("kOtelSpan");
const kOtelStartTime = Symbol("kOtelStartTime");
const kOtelReqBodySize = Symbol("kOtelReqBodySize");

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
const _kOnTimeout = HTTPParser.kOnTimeout | 0;

// JS-based ConnectionsList matching Node's native ConnectionsList.
// Tracks active connections and their request start times for
// headersTimeout / requestTimeout enforcement.
class ConnectionsList {
  constructor() {
    this._all = new Set();
    this._active = new Map(); // socket -> { headersCompleted, startTime }
  }

  push(socket) {
    this._all.add(socket);
  }

  pop(socket) {
    this._all.delete(socket);
    this._active.delete(socket);
  }

  pushActive(socket) {
    this._active.set(socket, {
      headersCompleted: false,
      startTime: performance.now(),
    });
  }

  popActive(socket) {
    this._active.delete(socket);
  }

  markHeadersCompleted(socket) {
    const entry = this._active.get(socket);
    if (entry) {
      entry.headersCompleted = true;
    }
  }

  expired(headersTimeout, requestTimeout) {
    const now = performance.now();
    const result = [];

    for (const [socket, entry] of this._active) {
      const elapsed = now - entry.startTime;
      if (!entry.headersCompleted && headersTimeout > 0) {
        if (elapsed >= headersTimeout) {
          result.push({ socket });
          continue;
        }
      }
      if (requestTimeout > 0 && elapsed >= requestTimeout) {
        result.push({ socket });
      }
    }

    return result;
  }
}

function onRequestTimeout(socket) {
  socketOnError.call(socket, new Error("ERR_HTTP_REQUEST_TIMEOUT"));
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
  OutgoingMessage.call(this, options);

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
        throw new ERR_INVALID_ARG_TYPE(
          "headers",
          "Array with even length",
          obj,
        );
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
  connections.push(socket);
  const onConnectionClose = () => connections.pop(socket);
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
  };
  state.onData = socketOnData.bind(undefined, server, socket, parser, state);
  state.onEnd = socketOnEnd.bind(undefined, server, socket, parser, state);
  state.onClose = socketOnClose.bind(undefined, socket, state);
  state.onDrain = socketOnDrain.bind(undefined, socket, state);
  socket.on("data", state.onData);
  socket.on("error", socketOnError);
  socket.on("end", state.onEnd);
  socket.on("close", state.onClose);
  socket.on("drain", state.onDrain);
  parser.onIncoming = parserOnIncoming.bind(
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
  parser[kOnExecute] = onParserExecute.bind(
    undefined,
    server,
    socket,
    parser,
    state,
  );

  socket._paused = false;
}

function onParserExecute(server, socket, parser, state, ret, d) {
  socket._unrefTimer?.();
  // The consume path (parser.consume(handle)) passes `d` as a bare
  // Uint8Array from the C++ binding. onParserExecuteCommon's upgrade
  // branch does `d.slice(bytesParsed).toString()` and expects the
  // Buffer `.toString(encoding)` semantics, not the plain Uint8Array
  // behavior. Wrap to match the non-consume path where `d` came from
  // `socket.on('data')` as a Buffer.
  if (d !== undefined && !Buffer.isBuffer(d)) {
    d = Buffer.from(d.buffer, d.byteOffset, d.byteLength);
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
  freeParser(socket.parser, undefined, socket);
  abortIncoming(state.incoming);
}

function abortIncoming(incoming) {
  while (incoming.length) {
    const req = incoming.shift();
    req.destroy(connResetException("aborted"));
  }
}

function socketOnEnd(server, socket, parser, state) {
  const ret = parser.finish();

  if (ret instanceof Error) {
    prepareError(ret, parser);
    socketOnError.call(socket, ret);
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

  const ret = parser.execute(d);

  onParserExecuteCommon(server, socket, parser, state, ret, d);
}

function googLength(d) {
  return d.length || d.byteLength;
}

function onParserExecuteCommon(server, socket, parser, state, ret, d) {
  resetSocketTimeout(server, socket, state);

  if (ret instanceof Error) {
    prepareError(ret, parser, d);
    socketOnError.call(socket, ret);
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
    if (connections) connections.pop(socket);

    parser.finish();
    freeParser(parser, req, socket);

    const bodyHead = d.slice(bytesParsed);

    socket.readableFlowing = null;
    server.emit(eventName, req, socket, bodyHead);
  }
}

function resetSocketTimeout(server, socket, state) {
  if (!state.keepAliveTimeoutSet) return;
  socket.setTimeout(server.timeout || 0);
  state.keepAliveTimeoutSet = false;
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

const badRequestResponse =
  "HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n";
const requestHeaderFieldsTooLargeResponse =
  "HTTP/1.1 431 Request Header Fields Too Large\r\nConnection: close\r\n\r\n";

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
  resetSocketTimeout(server, socket, state);

  if (req.upgrade) {
    req.upgrade = req.method === "CONNECT" || true;
    if (req.upgrade) return 0;
  }

  state.incoming.push(req);

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
  });
  res._keepAliveTimeout = server.keepAliveTimeout;
  res._maxRequestsPerSocket = server.maxRequestsPerSocket;
  res._onPendingData = updateOutgoingData.bind(undefined, socket, state);

  res.shouldKeepAlive = keepAlive;
  res[kUniqueHeaders] = server[kUniqueHeaders];

  // Start OTel server span and metrics
  if (TRACING_ENABLED) {
    // Extract trace context from incoming request headers
    let context = ContextManager.active();
    if (PROPAGATORS.length > 0) {
      for (const propagator of PROPAGATORS) {
        context = propagator.extract(context, req.headers, {
          get(carrier, key) {
            return carrier[key];
          },
          keys(carrier) {
            return Object.keys(carrier);
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
    span.setAttribute("url.path", url.split("?")[0]);
    span.setAttribute("url.query", url.includes("?") ? url.split("?")[1] : "");
    res[kOtelSpan] = span;
  }
  if (METRICS_ENABLED) {
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
    state.outgoing.push(res);
  } else {
    res.assignSocket(socket);
  }

  res.on(
    "finish",
    resOnFinish.bind(undefined, req, res, socket, state, server),
  );

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

  return 0;
}

function resOnFinish(req, res, socket, state, server) {
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

  state.incoming.shift();

  if (!req._consuming && !req._readableState?.resumeScheduled) {
    req._dump();
  }

  res.detachSocket(socket);
  clearIncoming(req);
  nextTick(emitCloseNT, res);

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
        socket.setTimeout(keepAliveTimeout + 1000);
        state.keepAliveTimeoutSet = true;
      }
    }
  } else {
    const m = state.outgoing.shift();
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
  if (!(this instanceof Server)) return new Server(options, requestListener);

  if (typeof options === "function") {
    requestListener = options;
    options = kEmptyObject;
  } else if (options == null) {
    options = kEmptyObject;
  } else {
    validateObject(options, "options");
  }

  storeHTTPOptions.call(this, options);

  net.Server.call(this, {
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
    checkConnections.bind(this),
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
}
ObjectSetPrototypeOf(Server.prototype, net.Server.prototype);
ObjectSetPrototypeOf(Server, net.Server);

Server.prototype.setTimeout = function setTimeout(msecs, callback) {
  this.timeout = msecs;
  if (callback) {
    this.on("timeout", callback);
  }
  return this;
};

Server.prototype.close = function close(cb) {
  httpServerPreClose(this);
  return net.Server.prototype.close.call(this, cb);
};

Server.prototype.closeAllConnections = function closeAllConnections() {
  const connections = this[kConnectionsKey];
  if (connections) {
    for (const socket of connections._all) {
      socket.destroy();
    }
  }
};

Server.prototype.closeIdleConnections = function closeIdleConnections() {
  const connections = this[kConnectionsKey];
  if (connections) {
    for (const socket of connections._all) {
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
