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

// Ported from Node.js lib/_http_client.js

// deno-lint-ignore-file prefer-primordials no-this-alias no-inner-declarations

import { core, internals, primordials } from "ext:core/mod.js";
const {
  ArrayIsArray,
  Boolean,
  DateNow,
  Error,
  NumberIsFinite,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectKeys,
  ObjectSetPrototypeOf,
  ReflectApply,
  String,
  Symbol,
} = primordials;

import net from "node:net";
const { ok: assert } = core.loadExtScript("ext:deno_node/assert.ts");
const { kEmptyObject, once } = core.loadExtScript(
  "ext:deno_node/internal/util.mjs",
);
import {
  _checkIsHttpToken as checkIsHttpToken,
  freeParser,
  HTTPParser,
  isLenient,
  kSkipPendingData,
  parsers,
  prepareError,
} from "node:_http_common";
import {
  kUniqueHeaders,
  OutgoingMessage,
  parseUniqueHeadersOption,
} from "node:_http_outgoing";
import httpAgent from "node:_http_agent";
import httpProxy from "node:_http_proxy";
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { urlToHttpOptions } = core.loadExtScript(
  "ext:deno_node/internal/url.ts",
);
const { kOutHeaders } = core.loadExtScript("ext:deno_node/internal/http.ts");
const {
  connResetException,
  ERR_HTTP_HEADERS_SENT,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_HTTP_TOKEN,
  ERR_INVALID_PROTOCOL,
  ERR_INVALID_URL,
  ERR_UNESCAPED_CHARACTERS,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  validateBoolean,
  validateInteger,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { getTimerDuration } = core.loadExtScript(
  "ext:deno_node/internal/timers.mjs",
);
import { addAbortSignal, finished } from "node:stream";
const { nextTick } = core.loadExtScript("ext:deno_node/_next_tick.ts");
const { defaultTriggerAsyncIdScope } = core.loadExtScript(
  "ext:deno_node/internal/async_hooks.ts",
);
const { kNeedDrain } = core.loadExtScript("ext:deno_node/internal/http.ts");
const { channel } = core.loadExtScript("ext:deno_node/diagnostics_channel.js");
const { enqueueNodePerformanceEntry } = core.loadExtScript(
  "ext:deno_node/perf_hooks.js",
);

const onClientRequestCreatedChannel = channel("http.client.request.created");
const onClientRequestStartChannel = channel("http.client.request.start");
const onClientRequestErrorChannel = channel("http.client.request.error");
const onClientResponseFinishChannel = channel("http.client.response.finish");
const { updateSpanFromError } = core.loadExtScript(
  "ext:deno_telemetry/util.ts",
);
const {
  otelState,
  builtinTracer,
  ContextManager,
  SPAN_KEY,
} = core.loadExtScript("ext:deno_telemetry/telemetry.ts");

const INVALID_PATH_REGEX = /[^\u0021-\u00ff]/;
const kError = Symbol("kError");
const kPath = Symbol("kPath");
const kOtelSpan = Symbol("kOtelSpan");
const kPerfStartTime = Symbol("kPerfStartTime");
const kRetryData = Symbol("kRetryData");
const kRetryOptions = Symbol("kRetryOptions");
const kProxy = Symbol("kProxy");
const kProxyTargetHost = Symbol("kProxyTargetHost");
const kProxyTargetPort = Symbol("kProxyTargetPort");
const kInspectorRequestId = Symbol("kInspectorRequestId");
const kInspectorNetwork = Symbol("kInspectorNetwork");
const kInspectorUrl = Symbol("kInspectorUrl");
const kInspectorCompleted = Symbol("kInspectorCompleted");

// ============================================================================
// Inspector Network domain instrumentation (Chrome DevTools Protocol).
//
// When `node:inspector` has been loaded and `--inspect` is active, node:http
// client requests emit `Network.requestWillBeSent` / `responseReceived` /
// `dataReceived` / `loadingFinished` / `loadingFailed` events. The emitters
// and a monotonic requestId generator are installed by
// `ext/node/polyfills/inspector.js` onto `internals.__inspectorNetwork`,
// so this layer is one `isEnabled()` check when the inspector is detached.
//
// Mirrors the implementation in ext/fetch/26_fetch.js. Differences:
//   - `type: "Other"` rather than `"Fetch"` (matches Chrome DevTools' contract
//     for non-fetch HTTP).
//   - Request headers are read from `kOutHeaders` rather than a flat list.
//   - Response body bytes are observed by wrapping `res.push`, not by teeing
//     a ReadableStream, since IncomingMessage is a Node Readable. The wrapper
//     stays out of the read pipeline so the response remains paused until the
//     consumer attaches a `data` listener (asserted by the upstream test).
// ============================================================================
function getInspectorNetwork() {
  const ins = internals.__inspectorNetwork;
  if (ins && ins.isEnabled()) return ins;
  return null;
}

function safeEmit(fn, params) {
  try {
    fn(params);
  } catch {
    // Inspector emission is purely observational - never surface as an
    // http error to user code.
  }
}

function joinRequestHeadersForCdp(req) {
  const out = { __proto__: null };
  const headers = req[kOutHeaders];
  if (!headers) return out;
  const keys = ObjectKeys(headers);
  for (let i = 0; i < keys.length; i++) {
    const lower = keys[i]; // kOutHeaders keys are already lowercase
    const entry = headers[lower];
    if (!entry) continue;
    const value = entry[1];
    let separator;
    if (lower === "cookie") {
      separator = "; ";
    } else if (lower === "set-cookie") {
      separator = "\n";
    } else {
      separator = ", ";
    }
    if (ArrayIsArray(value)) {
      let joined = "";
      for (let j = 0; j < value.length; j++) {
        if (j > 0) joined += separator;
        joined += String(value[j]);
      }
      out[lower] = joined;
    } else {
      out[lower] = String(value);
    }
  }
  return out;
}

function joinResponseHeadersForCdp(rawHeaders) {
  const out = { __proto__: null };
  if (!rawHeaders) return out;
  for (let i = 0; i < rawHeaders.length; i += 2) {
    const rawName = rawHeaders[i];
    const value = String(rawHeaders[i + 1]);
    const lower = String(rawName).toLowerCase();
    let separator;
    if (lower === "cookie") {
      separator = "; ";
    } else if (lower === "set-cookie") {
      separator = "\n";
    } else {
      separator = ", ";
    }
    if (out[lower] === undefined) {
      out[lower] = value;
    } else {
      out[lower] = out[lower] + separator + value;
    }
  }
  return out;
}

function parseContentTypeFromRawHeaders(rawHeaders) {
  let raw = null;
  if (rawHeaders) {
    for (let i = 0; i < rawHeaders.length; i += 2) {
      if (String(rawHeaders[i]).toLowerCase() === "content-type") {
        raw = String(rawHeaders[i + 1]);
        break;
      }
    }
  }
  if (raw === null) return { mimeType: "", charset: "" };
  const semi = raw.indexOf(";");
  const mimeType = semi === -1 ? raw.trim() : raw.slice(0, semi).trim();
  let charset = "";
  if (semi !== -1) {
    const rest = raw.slice(semi + 1);
    const parts = rest.split(";");
    for (let i = 0; i < parts.length; i++) {
      const p = parts[i].trim();
      if (p.toLowerCase().startsWith("charset=")) {
        charset = p.slice(8).trim();
        if (
          charset.length >= 2 && charset[0] === '"' &&
          charset[charset.length - 1] === '"'
        ) {
          charset = charset.slice(1, charset.length - 1);
        }
        break;
      }
    }
  }
  return { mimeType, charset };
}

function buildInspectorRequestUrl(protocol, host, port, path) {
  let hostPart = host || "localhost";
  if (hostPart.indexOf(":") !== -1 && hostPart.charCodeAt(0) !== 91 /* '[' */) {
    hostPart = `[${hostPart}]`;
  }
  const proto = protocol || "http:";
  const defaultPort = proto === "https:" ? 443 : 80;
  const portStr = port && +port !== defaultPort ? `:${port}` : "";
  return `${proto}//${hostPart}${portStr}${path || "/"}`;
}

function inspectorEmitRequestWillBeSent(req, port) {
  const ins = getInspectorNetwork();
  if (!ins) return;
  const requestId = ins.nextRequestId();
  req[kInspectorRequestId] = requestId;
  req[kInspectorNetwork] = ins;
  const url = buildInspectorRequestUrl(
    req.protocol,
    req.host,
    port,
    req[kPath],
  );
  req[kInspectorUrl] = url;
  const headers = joinRequestHeadersForCdp(req);
  const now = DateNow() / 1000;
  safeEmit(ins.requestWillBeSent, {
    requestId,
    timestamp: now,
    wallTime: now,
    type: "Other",
    request: {
      url,
      method: req.method,
      headers,
      hasPostData: false,
    },
  });
}

function inspectorEmitResponseReceived(req, res) {
  const ins = req[kInspectorNetwork];
  const requestId = req[kInspectorRequestId];
  if (!ins || !requestId) return;
  const rawHeaders = res.rawHeaders;
  const headers = joinResponseHeadersForCdp(rawHeaders);
  const { mimeType, charset } = parseContentTypeFromRawHeaders(rawHeaders);
  safeEmit(ins.responseReceived, {
    requestId,
    timestamp: DateNow() / 1000,
    type: "Other",
    response: {
      url: req[kInspectorUrl],
      status: res.statusCode,
      statusText: res.statusMessage || "",
      headers,
      mimeType,
      charset,
    },
  });

  // Wrap res.push to observe body chunks and the end-of-stream sentinel
  // (parserOnBody -> stream.push(buf); parserOnMessageComplete ->
  // stream.push(null)). Going through `data` events would put the stream
  // in flowing mode, breaking the upstream test that requires it to stay
  // paused until the consumer attaches a `data` listener.
  let totalLength = 0;
  const origPush = res.push;
  res.push = function inspectorPush(chunk, encoding) {
    if (chunk === null) {
      if (!req[kInspectorCompleted]) {
        req[kInspectorCompleted] = true;
        safeEmit(ins.loadingFinished, {
          requestId,
          timestamp: DateNow() / 1000,
          encodedDataLength: totalLength,
        });
      }
    } else if (chunk) {
      let len;
      if (typeof chunk === "string") {
        len = chunk.length;
      } else if (chunk.byteLength !== undefined) {
        len = chunk.byteLength;
      } else {
        len = 0;
      }
      if (len > 0) {
        totalLength += len;
        safeEmit(ins.dataReceived, {
          requestId,
          timestamp: DateNow() / 1000,
          dataLength: len,
          encodedDataLength: len,
          data: chunk,
        });
      }
    }
    return origPush.call(this, chunk, encoding);
  };
}

function inspectorEmitLoadingFailed(req, error) {
  const ins = req[kInspectorNetwork];
  const requestId = req[kInspectorRequestId];
  if (!ins || !requestId || req[kInspectorCompleted]) return;
  req[kInspectorCompleted] = true;
  let errorText;
  if (error && typeof error === "object" && error.message !== undefined) {
    errorText = String(error.message);
  } else {
    errorText = String(error);
  }
  safeEmit(ins.loadingFailed, {
    requestId,
    timestamp: DateNow() / 1000,
    type: "Other",
    errorText,
  });
}

const kLenientAll = HTTPParser.kLenientAll | 0;
const kLenientNone = HTTPParser.kLenientNone | 0;

class HTTPClientAsyncResource {
  constructor(type, req) {
    this.type = type;
    this.req = req;
  }
}

function validateHost(host, name) {
  if (host !== null && host !== undefined && typeof host !== "string") {
    throw new ERR_INVALID_ARG_TYPE(
      `options.${name}`,
      ["string", "undefined", "null"],
      host,
    );
  }
  return host;
}

function emitErrorEvent(request, error) {
  if (onClientRequestErrorChannel.hasSubscribers) {
    onClientRequestErrorChannel.publish({
      request,
      error,
    });
  }
  // ---- Inspector: Network.loadingFailed ----------------------------------
  // Fired before the user's `error` listener so DevTools sees the failure
  // even if the listener throws.
  inspectorEmitLoadingFailed(request, error);
  request.emit("error", error);
}

function isURL(input) {
  return input instanceof URL;
}

function ClientRequest(input, options, cb) {
  OutgoingMessage.call(this);

  if (typeof input === "string") {
    const urlStr = input;
    // Match Node: `new URL(...)` in ClientRequest surfaces as
    // ERR_INVALID_URL (node's internal URL constructor calls
    // bindingUrl.parse with raiseException=true). Deno's Web URL
    // throws a generic TypeError, so wrap it to attach the code.
    let parsed;
    try {
      parsed = new URL(urlStr);
    } catch {
      throw new ERR_INVALID_URL(urlStr);
    }
    input = urlToHttpOptions(parsed);
  } else if (isURL(input)) {
    input = urlToHttpOptions(input);
  } else {
    cb = options;
    options = input;
    input = null;
  }

  if (typeof options === "function") {
    cb = options;
    options = input || kEmptyObject;
  } else {
    options = ObjectAssign(input || {}, options);
  }

  let agent = options.agent;
  const defaultAgent = options._defaultAgent || httpAgent.globalAgent;
  if (agent === false) {
    agent = new defaultAgent.constructor();
  } else if (agent === null || agent === undefined) {
    if (typeof options.createConnection !== "function") {
      agent = defaultAgent;
    }
  } else if (typeof agent.addRequest !== "function") {
    throw new ERR_INVALID_ARG_TYPE(
      "options.agent",
      ["Agent-like Object", "undefined", "false"],
      agent,
    );
  }
  this.agent = agent;

  const protocol = options.protocol || defaultAgent.protocol;
  let expectedProtocol = defaultAgent.protocol;
  if (this.agent?.protocol) {
    expectedProtocol = this.agent.protocol;
  }

  if (options.path) {
    const path = String(options.path);
    if (INVALID_PATH_REGEX.test(path)) {
      throw new ERR_UNESCAPED_CHARACTERS("Request path");
    }
  }

  if (protocol !== expectedProtocol) {
    throw new ERR_INVALID_PROTOCOL(protocol, expectedProtocol);
  }

  const defaultPort = options.defaultPort ||
    (this.agent?.defaultPort);

  const optsWithoutSignal = { __proto__: null, ...options };

  const port = optsWithoutSignal.port = options.port || defaultPort || 80;
  const host = optsWithoutSignal.host =
    validateHost(options.hostname, "hostname") ||
    validateHost(options.host, "host") || "localhost";

  // Proxy detection: if an env-derived proxy applies to this request,
  // either rewrite to absolute URL (http target) or set up a CONNECT tunnel
  // via a custom createConnection (https target). The agent's socket pool
  // is keyed by target host:port so users can look it up the same way as
  // a direct connection - the proxy is a transport detail tracked under
  // _proxy on the options.
  const proxyConfig = httpProxy.resolveAgentProxyConfig(this.agent);
  const proxyEntry = httpProxy.selectProxy(
    proxyConfig,
    protocol,
    host,
    port,
  );
  if (proxyEntry) {
    this[kProxy] = proxyEntry;
    this[kProxyTargetHost] = host;
    this[kProxyTargetPort] = port;
    optsWithoutSignal._proxy = proxyEntry;
    optsWithoutSignal._proxyTargetHost = host;
    optsWithoutSignal._proxyTargetPort = port;
    optsWithoutSignal._proxyProtocol = protocol;
    optsWithoutSignal._proxyUseProxyConnection =
      !(this.agent && this.agent.__proxyConfig !== undefined) ||
      this.agent?.keepAlive === true;
  }

  const setHost = options.setHost !== undefined
    ? Boolean(options.setHost)
    : options.setDefaultHeaders !== false;

  this._removedConnection = options.setDefaultHeaders === false;
  this._removedContLen = options.setDefaultHeaders === false;
  this._removedTE = options.setDefaultHeaders === false;

  this.socketPath = options.socketPath;

  if (options.timeout !== undefined) {
    this.timeout = getTimerDuration(options.timeout, "timeout");
  }

  const signal = options.signal;
  if (signal) {
    addAbortSignal(signal, this);
    delete optsWithoutSignal.signal;
  }
  let method = options.method;
  if (method != null) {
    if (typeof method !== "string") {
      throw new ERR_INVALID_ARG_TYPE("options.method", "string", method);
    }
  }

  if (method) {
    if (!checkIsHttpToken(method)) {
      throw new ERR_INVALID_HTTP_TOKEN("Method", method);
    }
    method = this.method = method.toUpperCase();
  } else {
    method = this.method = "GET";
  }

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

  if (options.joinDuplicateHeaders !== undefined) {
    validateBoolean(
      options.joinDuplicateHeaders,
      "options.joinDuplicateHeaders",
    );
  }
  this.joinDuplicateHeaders = options.joinDuplicateHeaders;

  this[kPath] = options.path || "/";

  // For HTTP via HTTP proxy, rewrite path to an absolute URL so the proxy
  // knows where to forward the request.
  if (this[kProxy] && protocol === "http:") {
    const t = this[kProxyTargetHost];
    const formattedHost = t && t.indexOf(":") !== -1 && t.charCodeAt(0) !== 91
      ? `[${t}]`
      : t;
    this[kPath] = `http://${formattedHost}:${this[kProxyTargetPort]}${
      options.path || "/"
    }`;
  }

  if (cb) {
    this.once("response", cb);
  }

  if (
    method === "GET" ||
    method === "HEAD" ||
    method === "DELETE" ||
    method === "OPTIONS" ||
    method === "TRACE" ||
    method === "CONNECT"
  ) {
    this.useChunkedEncodingByDefault = false;
  } else {
    this.useChunkedEncodingByDefault = true;
  }

  this._ended = false;
  this.res = null;
  this.aborted = false;
  this.timeoutCb = null;
  this.upgradeOrConnect = false;
  this.parser = null;
  this.maxHeadersCount = null;
  this.reusedSocket = false;
  this.host = host;
  this.protocol = protocol;
  this[kPerfStartTime] = performance.now();

  if (this.agent) {
    if (!this.agent.keepAlive && !NumberIsFinite(this.agent.maxSockets)) {
      this._last = true;
      this.shouldKeepAlive = false;
    } else {
      this._last = false;
      this.shouldKeepAlive = true;
    }
  }

  const headersArray = ArrayIsArray(options.headers);
  if (!headersArray) {
    if (options.headers) {
      const keys = ObjectKeys(options.headers);
      for (let i = 0; i < keys.length; i++) {
        const key = keys[i];
        this.setHeader(key, options.headers[key]);
      }
    }

    if (host && !this.getHeader("host") && setHost) {
      let hostHeader = host;

      const posColon = hostHeader.indexOf(":");
      if (
        posColon !== -1 &&
        hostHeader.includes(":", posColon + 1) &&
        hostHeader.charCodeAt(0) !== 91 /* '[' */
      ) {
        hostHeader = `[${hostHeader}]`;
      }

      if (port && +port !== defaultPort) {
        hostHeader += ":" + port;
      }
      this.setHeader("Host", hostHeader);
    }

    if (options.auth && !this.getHeader("Authorization")) {
      this.setHeader(
        "Authorization",
        "Basic " + Buffer.from(options.auth).toString("base64"),
      );
    }

    if (this[kProxy] && protocol === "http:") {
      // Mirror what _storeHeader will pick for Connection: when shouldKeepAlive
      // is true, both Connection and Proxy-Connection are "keep-alive"; when
      // false, both are "close". Matches Node's wire format on the proxy hop.
      if (!this.getHeader("proxy-connection")) {
        this.setHeader(
          "Proxy-Connection",
          this.shouldKeepAlive ? "keep-alive" : "close",
        );
      }
      if (this[kProxy].auth && !this.getHeader("proxy-authorization")) {
        this.setHeader("Proxy-Authorization", this[kProxy].auth);
      }
    }

    if (this.getHeader("expect")) {
      if (this._header) {
        throw new ERR_HTTP_HEADERS_SENT("render");
      }

      this._storeHeader(
        this.method + " " + this.path + " HTTP/1.1\r\n",
        this[kOutHeaders],
      );
    }
  } else {
    this._storeHeader(
      this.method + " " + this.path + " HTTP/1.1\r\n",
      options.headers,
    );
  }

  this[kUniqueHeaders] = parseUniqueHeadersOption(options.uniqueHeaders);

  // Save options for potential stale keepalive retry
  this[kRetryOptions] = optsWithoutSignal;

  // initiate connection
  if (this.agent) {
    this.agent.addRequest(this, optsWithoutSignal);
  } else {
    this._last = true;
    this.shouldKeepAlive = false;
    let opts = optsWithoutSignal;
    if (opts.path || opts.socketPath) {
      opts = { ...optsWithoutSignal };
      if (opts.socketPath) {
        opts.path = opts.socketPath;
      } else {
        opts.path &&= undefined;
      }
    }
    if (typeof opts.createConnection === "function") {
      const oncreate = once((err, socket) => {
        if (err) {
          nextTick(() => emitErrorEvent(this, err));
        } else {
          this.onSocket(socket);
        }
      });

      try {
        const newSocket = opts.createConnection(opts, oncreate);
        if (newSocket) {
          oncreate(null, newSocket);
        }
      } catch (err) {
        oncreate(err);
      }
    } else {
      this.onSocket(net.createConnection(opts));
    }
  }
  if (onClientRequestCreatedChannel.hasSubscribers) {
    onClientRequestCreatedChannel.publish({
      request: this,
    });
  }
  // ---- Inspector: Network.requestWillBeSent ------------------------------
  // Fire here so the user-code call site (e.g. `http.get(...)`) is still on
  // the stack - `op_inspector_emit_protocol_event` captures it as the
  // `initiator` for DevTools. Any setHeader() the caller does between the
  // constructor returning and `req.end()` would be missed; the upstream
  // node:http inspector implementation behaves the same way.
  inspectorEmitRequestWillBeSent(this, port);
}
ObjectSetPrototypeOf(ClientRequest.prototype, OutgoingMessage.prototype);
ObjectSetPrototypeOf(ClientRequest, OutgoingMessage);

ObjectDefineProperty(ClientRequest.prototype, "path", {
  __proto__: null,
  get() {
    return this[kPath];
  },
  set(value) {
    const path = String(value);
    if (INVALID_PATH_REGEX.test(path)) {
      throw new ERR_UNESCAPED_CHARACTERS("Request path");
    }
    this[kPath] = path;
  },
  configurable: true,
  enumerable: true,
});

ClientRequest.prototype._finish = function _finish() {
  OutgoingMessage.prototype._finish.call(this);
  if (onClientRequestStartChannel.hasSubscribers) {
    onClientRequestStartChannel.publish({
      request: this,
    });
  }
};

ClientRequest.prototype._implicitHeader = function _implicitHeader() {
  if (this._header) {
    throw new ERR_HTTP_HEADERS_SENT("render");
  }

  // Start OTel client span and inject propagation headers before serialization
  if (otelState.TRACING_ENABLED && !this[kOtelSpan]) {
    const span = builtinTracer().startSpan(this.method, { kind: 2 }); // Kind 2 = Client
    this[kOtelSpan] = span;

    // Build a context with this span for propagation injection,
    // without entering it into the async context
    const spanContext = ContextManager.active().setValue(SPAN_KEY, span);
    for (const propagator of otelState.PROPAGATORS) {
      propagator.inject(spanContext, this, {
        set(carrier, key, value) {
          carrier.setHeader(key, value);
        },
      });
    }

    // Set request attributes
    const protocol = this.protocol || "http:";
    const host = this.getHeader("host") || this.host || "localhost";
    const path = this.path || "/";
    const fullUrl = `${protocol}//${host}${path}`;
    try {
      const parsedUrl = new URL(fullUrl);
      span.setAttribute("http.request.method", this.method);
      span.setAttribute("url.full", parsedUrl.href);
      span.setAttribute("url.scheme", parsedUrl.protocol.slice(0, -1));
      span.setAttribute("url.path", parsedUrl.pathname);
      span.setAttribute("url.query", parsedUrl.search.slice(1));
    } catch {
      span.setAttribute("http.request.method", this.method);
      span.setAttribute("url.full", fullUrl);
    }
  }

  this._storeHeader(
    this.method + " " + this.path + " HTTP/1.1\r\n",
    this[kOutHeaders],
  );
};

ClientRequest.prototype.abort = function abort() {
  if (this.aborted) {
    return;
  }
  this.aborted = true;
  nextTick(emitAbortNT, this);
  this.destroy();
};

ClientRequest.prototype.destroy = function destroy(err) {
  if (this.destroyed) {
    return this;
  }
  this.destroyed = true;

  if (this.res) {
    this.res._dump();
  }

  this[kError] = err;
  this.socket?.destroy(err);

  return this;
};

function emitAbortNT(req) {
  req.emit("abort");
}

function ondrain() {
  const msg = this._httpMessage;
  if (msg && !msg.finished && msg[kNeedDrain]) {
    msg[kNeedDrain] = false;
    msg.emit("drain");
  }
}

// Transparently retry a request on a new connection when the reused
// keepalive socket turns out to be stale (server closed it while idle).
function maybeRetryRequest(req, socket) {
  if (!req.reusedSocket || req.res || !req.agent || req._retrying) {
    return false;
  }

  req._retrying = true;
  const agent = req.agent;

  // Clean up parser on the old socket
  const parser = socket.parser;
  if (parser) {
    parser.finish();
    freeParser(parser, req, socket);
  }

  // Remove listeners installed by tickOnSocket
  socket.removeListener("close", socketCloseListener);
  socket.removeListener("error", socketErrorListener);
  socket.removeListener("data", socketOnData);
  socket.removeListener("end", socketOnEnd);
  socket.removeListener("drain", ondrain);
  if (req.timeoutCb) {
    socket.removeListener("timeout", req.timeoutCb);
  }
  socket.removeListener("timeout", responseOnTimeout);

  // Remove the stale socket from the agent's active sockets so that
  // addRequest sees room under maxSockets to create a new connection.
  // The installListeners onClose handler will still fire after destroy()
  // and handle totalSocketCount bookkeeping.
  const retryOpts = req[kRetryOptions];
  const name = agent.getName(retryOpts);
  const sockets = agent.sockets[name];
  if (sockets) {
    const idx = sockets.indexOf(socket);
    if (idx !== -1) sockets.splice(idx, 1);
    if (!sockets.length) delete agent.sockets[name];
  }

  socket.destroy();

  // Reset request state for retry.
  // Keep finished as-is: if end() was called, _flush() on the new socket
  // needs to call _finish() to emit 'finish' (which pipeline awaits).
  req.socket = null;
  req.parser = null;
  req._header = null;
  req._headerSent = false;
  req.destroyed = false;
  req._closed = false;
  // The first attempt set reusedSocket on the stale socket. The retry runs
  // through addRequest again, which will call agent.reuseSocket() if it
  // happens to land on another pooled free socket. Otherwise the request
  // goes to a brand-new connection and the flag must stay false.
  req.reusedSocket = false;

  // Restore output data saved before the first flush attempt
  if (req[kRetryData]) {
    req.outputData = req[kRetryData];
    req[kRetryData] = null;
  }

  // Re-queue through agent to get a fresh socket
  agent.addRequest(req, retryOpts);
  return true;
}

function socketCloseListener() {
  const socket = this;
  const req = socket._httpMessage;

  // Guard against close firing on a socket that has no associated request.
  if (!req) {
    return;
  }

  const parser = socket.parser;
  const res = req.res;

  // Retry on stale keepalive socket before any error/cleanup handling
  if (!res && maybeRetryRequest(req, socket)) return;

  // End OTel span on socket close
  const span = req[kOtelSpan];
  if (span) {
    if (!res || !res.complete) {
      updateSpanFromError(span, connResetException("socket hang up"));
    }
    span.end();
    req[kOtelSpan] = null;
  }

  req.destroyed = true;
  if (res) {
    if (!res.complete) {
      res.destroy(connResetException("aborted"));
    }
    req._closed = true;
    req.emit("close");
    if (!res.aborted && res.readable) {
      res.push(null);
    }
  } else {
    if (!req.socket._hadError) {
      req.socket._hadError = true;
      emitErrorEvent(req, connResetException("socket hang up"));
    }
    req._closed = true;
    req.emit("close");
  }

  if (req.outputData) {
    req.outputData.length = 0;
  }

  if (parser) {
    parser.finish();
    freeParser(parser, req, socket);
  }
}

function socketErrorListener(err) {
  const socket = this;
  const req = socket._httpMessage;

  if (req) {
    // Retry on stale keepalive socket before emitting error
    if (maybeRetryRequest(req, socket)) return;

    // End OTel span on error
    const span = req[kOtelSpan];
    if (span) {
      updateSpanFromError(span, err);
      span.end();
      req[kOtelSpan] = null;
    }
    socket._hadError = true;
    emitErrorEvent(req, err);
  }

  const parser = socket.parser;
  if (parser) {
    parser.finish();
    freeParser(parser, req, socket);
  }

  socket.removeListener("data", socketOnData);
  socket.removeListener("end", socketOnEnd);
  socket.destroy();
}

function socketOnEnd() {
  const socket = this;
  const req = this._httpMessage;
  const parser = this.parser;

  // Retry on stale keepalive socket (server sent FIN while idle)
  if (!req.res && maybeRetryRequest(req, socket)) return;

  if (!req.res && !req.socket._hadError) {
    req.socket._hadError = true;
    emitErrorEvent(req, connResetException("socket hang up"));
  }
  if (parser) {
    parser.finish();
    freeParser(parser, req, socket);
  }
  socket.destroy();
}

function socketOnData(d) {
  const socket = this;
  const req = this._httpMessage;
  const parser = this.parser;

  assert(parser && parser.socket === socket);

  const ret = parser.execute(d);
  if (ret instanceof Error) {
    prepareError(ret, parser, d);
    freeParser(parser, req, socket);
    socket.removeListener("data", socketOnData);
    socket.removeListener("end", socketOnEnd);
    socket.destroy();
    req.socket._hadError = true;
    emitErrorEvent(req, ret);
  } else if (parser.incoming?.upgrade) {
    // Upgrade (if status code 101) or CONNECT
    const bytesParsed = ret;
    const res = parser.incoming;
    req.res = res;

    socket.removeListener("data", socketOnData);
    socket.removeListener("end", socketOnEnd);
    socket.removeListener("drain", ondrain);

    if (req.timeoutCb) socket.removeListener("timeout", req.timeoutCb);
    socket.removeListener("timeout", responseOnTimeout);

    parser.finish();
    freeParser(parser, req, socket);

    const bodyHead = d.slice(bytesParsed, d.length);

    const eventName = req.method === "CONNECT" ? "connect" : "upgrade";
    if (req.listenerCount(eventName) > 0) {
      req.upgradeOrConnect = true;

      socket.emit("agentRemove");
      socket.removeListener("close", socketCloseListener);
      socket.removeListener("error", socketErrorListener);

      socket._httpMessage = null;
      socket.readableFlowing = null;

      req.emit(eventName, res, socket, bodyHead);
      req.destroyed = true;
      req._closed = true;
      req.emit("close");
    } else {
      socket.destroy();
    }
  } else if (
    parser.incoming?.complete &&
    !statusIsInformational(parser.incoming.statusCode)
  ) {
    socket.removeListener("data", socketOnData);
    socket.removeListener("end", socketOnEnd);
    socket.removeListener("drain", ondrain);
    freeParser(parser, req, socket);
  }
}

function statusIsInformational(status) {
  return (status < 200 && status >= 100 && status !== 101);
}

// client
function parserOnIncomingClient(res, shouldKeepAlive) {
  const socket = this.socket;
  const req = socket._httpMessage;

  if (req.res) {
    socket.destroy();
    if (socket.parser) {
      socket.parser.incoming = req.res;
      socket.parser.incoming[kSkipPendingData] = true;
    }
    return 0;
  }
  req.res = res;

  if (res.upgrade) return 2;

  const method = req.method;
  if (method === "CONNECT") {
    res.upgrade = true;
    return 2;
  }

  if (statusIsInformational(res.statusCode)) {
    req.res = null;
    if (res.statusCode === 100) {
      req.emit("continue");
    }
    req.emit("information", {
      statusCode: res.statusCode,
      statusMessage: res.statusMessage,
      httpVersion: res.httpVersion,
      httpVersionMajor: res.httpVersionMajor,
      httpVersionMinor: res.httpVersionMinor,
      headers: res.headers,
      rawHeaders: res.rawHeaders,
    });

    return 1;
  }

  if (req.shouldKeepAlive && !shouldKeepAlive && !req.upgradeOrConnect) {
    req.shouldKeepAlive = false;
  }

  req.res = res;
  res.req = req;

  // Emit HttpClient perf entry (at response-header time)
  const perfStartTime = req[kPerfStartTime];
  if (perfStartTime !== undefined) {
    const host = req.getHeader("host") || req.host || "localhost";
    enqueueNodePerformanceEntry({
      name: "HttpClient",
      entryType: "http",
      startTime: perfStartTime,
      duration: performance.now() - perfStartTime,
      detail: {
        req: {
          method: req.method,
          url: `${req.protocol || "http:"}//${host}${req.path || "/"}`,
          headers: req.getHeaders(),
        },
        res: {
          statusCode: res.statusCode,
          statusMessage: res.statusMessage || "",
          headers: res.headers,
        },
      },
    });
  }

  if (onClientResponseFinishChannel.hasSubscribers) {
    onClientResponseFinishChannel.publish({
      request: req,
      response: res,
    });
  }

  // ---- Inspector: Network.responseReceived -------------------------------
  // Also installs the `res.push` wrapper that emits `Network.dataReceived`
  // and `Network.loadingFinished` once body bytes start flowing.
  inspectorEmitResponseReceived(req, res);

  // Set OTel response attributes
  const span = req[kOtelSpan];
  if (span) {
    span.setAttribute("http.response.status_code", res.statusCode);
    if (res.statusCode >= 400) {
      span.setAttribute("error.type", String(res.statusCode));
      span.setStatus({ code: 2 });
    }
  }

  // Add our listener first, so that we guarantee socket cleanup
  res.on("end", responseOnEnd);
  req.on("finish", requestOnFinish);
  socket.on("timeout", responseOnTimeout);

  if (req.aborted || !req.emit("response", res)) {
    res._dump();
  }

  if (method === "HEAD") return 1;

  if (res.statusCode === 304) {
    res.complete = true;
    return 1;
  }

  return 0;
}

// client
function responseKeepAlive(req) {
  const socket = req.socket;

  if (req.timeoutCb) {
    socket.setTimeout(0, req.timeoutCb);
    req.timeoutCb = null;
  }
  socket.removeListener("close", socketCloseListener);
  socket.removeListener("error", socketErrorListener);
  socket.removeListener("data", socketOnData);
  socket.removeListener("end", socketOnEnd);

  // There are cases where _handle === null. Avoid those. Passing undefined to
  // nextTick() will call getDefaultTriggerAsyncId() to retrieve the id.
  const asyncId = socket._handle ? socket._handle.getAsyncId() : undefined;
  defaultTriggerAsyncIdScope(asyncId, nextTick, emitFreeNT, req);

  req.destroyed = true;
  if (req.res) {
    // Detach socket from IncomingMessage to avoid destroying the freed
    // socket in IncomingMessage.destroy().
    req.res.socket = null;
  }
}

function responseOnEnd() {
  const req = this.req;
  const socket = req.socket;

  if (socket) {
    if (req.timeoutCb) socket.removeListener("timeout", emitRequestTimeout);
    socket.removeListener("timeout", responseOnTimeout);
  }

  // End OTel client span
  const span = req[kOtelSpan];
  if (span) {
    span.end();
    req[kOtelSpan] = null;
  }

  req._ended = true;

  if (!req.shouldKeepAlive) {
    if (socket.writable) {
      if (typeof socket.destroySoon === "function") {
        socket.destroySoon();
      } else {
        socket.end();
      }
    }
    assert(!socket.writable);
  } else if (req.writableFinished && !this.aborted) {
    assert(req.finished);
    responseKeepAlive(req);
  }
}

function responseOnTimeout() {
  const req = this._httpMessage;
  if (!req) return;
  const res = req.res;
  if (!res) return;
  res.emit("timeout");
}

function requestOnFinish() {
  const req = this;
  if (req.shouldKeepAlive && req._ended && !req.destroyed) {
    responseKeepAlive(req);
  }
}

function emitFreeNT(req) {
  req._closed = true;
  req.emit("close");
  if (req.socket) {
    req.socket.emit("free");
  }
}

function tickOnSocket(req, socket) {
  const parser = parsers.alloc();
  req.socket = socket;
  const lenient = req.insecureHTTPParser === undefined
    ? isLenient()
    : req.insecureHTTPParser;
  parser.initialize(
    HTTPParser.RESPONSE,
    new HTTPClientAsyncResource("HTTPINCOMINGMESSAGE", req),
    req.maxHeaderSize || 0,
    lenient ? kLenientAll : kLenientNone,
  );
  parser.socket = socket;
  parser.outgoing = req;
  req.parser = parser;

  socket.parser = parser;
  socket._httpMessage = req;

  if (typeof req.maxHeadersCount === "number") {
    parser.maxHeaderPairs = req.maxHeadersCount << 1;
  }

  parser.joinDuplicateHeaders = req.joinDuplicateHeaders;

  parser.onIncoming = parserOnIncomingClient;
  socket.on("data", socketOnData);
  socket.on("end", socketOnEnd);
  socket.on("close", socketCloseListener);
  socket.on("drain", ondrain);

  if (
    req.timeout !== undefined ||
    (req.agent?.options?.timeout)
  ) {
    listenSocketTimeout(req);
  }
  req.emit("socket", socket);
}

function emitRequestTimeout() {
  const req = this._httpMessage;
  if (req) {
    req.emit("timeout");
  }
}

function listenSocketTimeout(req) {
  if (req.timeoutCb) {
    return;
  }
  req.timeoutCb = emitRequestTimeout;
  if (req.socket) {
    req.socket.once("timeout", emitRequestTimeout);
  } else {
    req.on("socket", (socket) => {
      socket.once("timeout", emitRequestTimeout);
    });
  }
}

ClientRequest.prototype.onSocket = function onSocket(socket, err) {
  if (socket && !err && socket.destroyed && socket.errored) {
    err = socket.errored;
  }
  if (socket && !err) {
    socket._httpMessage = this;
    socket.on("error", socketErrorListener);
  }
  nextTick(onSocketNT, this, socket, err);
};

function onSocketNT(req, socket, err) {
  if (req.destroyed || err) {
    req.destroyed = true;

    function _destroy(req, err) {
      if (!req.aborted && !err) {
        err = connResetException("socket hang up");
      }
      if (err && !socket?._hadError) {
        emitErrorEvent(req, err);
      }
      req._closed = true;
      req.emit("close");
    }

    if (socket) {
      if (!err && req.agent && !socket.destroyed) {
        socket.emit("free");
      } else {
        finished(socket.destroy(err || req[kError]), (er) => {
          if (er?.code === "ERR_STREAM_PREMATURE_CLOSE") {
            er = null;
          }
          _destroy(req, er || err);
        });
        return;
      }
    }

    _destroy(req, err || req[kError]);
  } else {
    tickOnSocket(req, socket);
    // Save output data before flushing so it can be replayed on retry
    // if this reused keepalive socket turns out to be stale.
    if (req.reusedSocket && req.outputData.length > 0) {
      req[kRetryData] = req.outputData.map((item) => ({
        data: item.data,
        encoding: item.encoding,
        callback: item.callback,
      }));
    }
    req._flush();
  }
}

ClientRequest.prototype._deferToConnect = _deferToConnect;
function _deferToConnect(method, arguments_) {
  const callSocketMethod = () => {
    if (method) {
      ReflectApply(this.socket[method], this.socket, arguments_);
    }
  };

  const onSocket = () => {
    if (this.socket.writable) {
      callSocketMethod();
    } else {
      this.socket.once("connect", callSocketMethod);
    }
  };

  if (!this.socket) {
    this.once("socket", onSocket);
  } else {
    onSocket();
  }
}

ClientRequest.prototype.setTimeout = function setTimeout(msecs, callback) {
  if (this._ended) {
    return this;
  }

  listenSocketTimeout(this);
  msecs = getTimerDuration(msecs, "msecs");
  if (callback) this.once("timeout", callback);

  if (this.socket) {
    setSocketTimeout(this.socket, msecs);
  } else {
    this.once("socket", (sock) => setSocketTimeout(sock, msecs));
  }

  return this;
};

function setSocketTimeout(sock, msecs) {
  if (sock.connecting) {
    sock.once("connect", function () {
      sock.setTimeout(msecs);
    });
  } else {
    sock.setTimeout(msecs);
  }
}

ClientRequest.prototype.setNoDelay = function setNoDelay(noDelay) {
  this._deferToConnect("setNoDelay", [noDelay]);
};

ClientRequest.prototype.setSocketKeepAlive = function setSocketKeepAlive(
  enable,
  initialDelay,
) {
  this._deferToConnect("setKeepAlive", [enable, initialDelay]);
};

ClientRequest.prototype.clearTimeout = function clearTimeout(cb) {
  this.setTimeout(0, cb);
};

export { ClientRequest };
export default { ClientRequest };
