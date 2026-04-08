// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core, primordials } from "ext:core/mod.js";
import {
  op_node_http_await_information,
  op_node_http_await_response,
  op_node_http_fetch_response_upgrade,
  op_node_http_request_with_conn,
  op_node_http_response_reclaim_conn,
  op_tls_key_null,
  op_tls_key_static,
  op_tls_start,
} from "ext:core/ops";

import { TextEncoder } from "ext:deno_web/08_text_encoding.js";
import { setTimeout } from "ext:deno_web/02_timers.js";
import { updateSpanFromError } from "ext:deno_telemetry/util.ts";
import {
  _normalizeArgs,
  createConnection,
  ListenOptions,
  Socket,
} from "node:net";
import { Buffer } from "node:buffer";
import { ERR_SERVER_NOT_RUNNING } from "ext:deno_node/internal/errors.ts";
import { EventEmitter } from "node:events";
import { nextTick } from "ext:deno_node/_next_tick.ts";
import {
  validateAbortSignal,
  validateBoolean,
  validateInteger,
  validateObject,
  validatePort,
} from "ext:deno_node/internal/validators.mjs";
import {
  addAbortSignal,
  Duplex as NodeDuplex,
  finished,
  Readable as NodeReadable,
  Writable as NodeWritable,
  WritableOptions as NodeWritableOptions,
} from "node:stream";
import {
  kUniqueHeaders,
  OutgoingMessage,
  parseUniqueHeadersOption,
  validateHeaderName,
  validateHeaderValue,
} from "node:_http_outgoing";
import { ok as assert } from "node:assert";
import { kOutHeaders } from "ext:deno_node/internal/http.ts";
import { _checkIsHttpToken as checkIsHttpToken } from "node:_http_common";
import { ClientRequest } from "node:_http_client";
import { Agent, globalAgent } from "node:_http_agent";
import { urlToHttpOptions } from "ext:deno_node/internal/url.ts";
import { kEmptyObject, once } from "ext:deno_node/internal/util.mjs";
import { constants, TCP } from "ext:deno_node/internal_binding/tcp_wrap.ts";
import { kStreamBaseField } from "ext:deno_node/internal_binding/stream_wrap.ts";
import { notImplemented } from "ext:deno_node/_utils.ts";
import {
  connResetException,
  ERR_HTTP_HEADERS_SENT,
  ERR_HTTP_SOCKET_ASSIGNED,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_HTTP_TOKEN,
  ERR_INVALID_PROTOCOL,
  ERR_UNESCAPED_CHARACTERS,
} from "ext:deno_node/internal/errors.ts";
import { getTimerDuration } from "ext:deno_node/internal/timers.mjs";
import { getIPFamily } from "ext:deno_node/internal/net.ts";
import {
  serveHttpOnListener,
  upgradeHttpRaw,
  upgradeHttpRawConnect,
} from "ext:deno_http/00_serve.ts";
import { op_http_serve_address_override } from "ext:core/ops";
import { listen as listenDeno } from "ext:deno_net/01_net.js";
import { headersEntries } from "ext:deno_fetch/20_headers.js";
import { Response } from "ext:deno_fetch/23_response.js";
import {
  builtinTracer,
  ContextManager,
  enterSpan,
  PROPAGATORS,
  restoreSnapshot,
  TRACING_ENABLED,
} from "ext:deno_telemetry/telemetry.ts";
import { timerId } from "ext:deno_web/03_abort_signal.js";
import { clearTimeout as webClearTimeout } from "ext:deno_web/02_timers.js";
import { resourceForReadableStream } from "ext:deno_web/06_streams.js";
import { kReinitializeHandle } from "ext:deno_node/internal/net.ts";
import {
  kDestroyed,
  kEnded,
  kEnding,
  kErrored,
  kState,
} from "ext:deno_node/internal/streams/utils.js";
import { TcpConn, UpgradedConn } from "ext:deno_net/01_net.js";
import { TlsConn } from "ext:deno_net/02_tls.js";
import {
  Server as ServerImpl,
  ServerResponse,
  STATUS_CODES,
} from "node:_http_server";
import { methods as METHODS } from "node:_http_common";
import { deprecate } from "node:util";

// Flag to track if DENO_SERVE_ADDRESS override has been consumed for Node http servers.
let nodeHttpAddressOverrideConsumed = false;

const { internalRidSymbol } = core;
const {
  ArrayIsArray,
  StringPrototypeIncludes,
  StringPrototypeToLowerCase,
  SafeArrayIterator,
} = primordials;

type Chunk = string | Buffer | Uint8Array;

const ENCODER = new TextEncoder();

export interface RequestOptions {
  agent?: Agent;
  auth?: string;
  createConnection?: () => unknown;
  defaultPort?: number;
  family?: number;
  headers?: Record<string, string>;
  hints?: number;
  host?: string;
  hostname?: string;
  insecureHTTPParser?: boolean;
  localAddress?: string;
  localPort?: number;
  lookup?: () => void;
  maxHeaderSize?: number;
  method?: string;
  path?: string;
  port?: number;
  protocol?: string;
  setHost?: boolean;
  socketPath?: string;
  timeout?: number;
  signal?: AbortSignal;
  href?: string;
}

function validateHost(host, name) {
  if (host !== null && host !== undefined && typeof host !== "string") {
    throw new ERR_INVALID_ARG_TYPE(`options.${name}`, [
      "string",
      "undefined",
      "null",
    ], host);
  }
  return host;
}

const INVALID_PATH_REGEX = /[^\u0021-\u00ff]/;
const kError = Symbol("kError");
const kBindToAbortSignal = Symbol("kBindToAbortSignal");

class FakeSocket extends EventEmitter {
  /** Stores the underlying request for lazily binding to abort signal */
  #request: Request | undefined;
  constructor(
    opts: {
      encrypted?: boolean | undefined;
      remotePort?: number | undefined;
      remoteAddress?: string | undefined;
      reader?: ReadableStreamDefaultReader | undefined;
      request?: Request;
    } = {},
  ) {
    super();
    this.remoteAddress = opts.remoteAddress;
    this.remotePort = opts.remotePort;
    this.encrypted = opts.encrypted;
    this.reader = opts.reader;
    this.writable = true;
    this.readable = true;
    this.#request = opts.request;
  }

  [kBindToAbortSignal]() {
    const signal = this.#request?.signal;
    signal?.addEventListener("abort", () => {
      this.emit("error", signal.reason);
      this.emit("close");
    }, { once: true });
  }

  setKeepAlive() {}

  end() {}

  destroy() {}

  setTimeout(callback, timeout = 0, ...args) {
    setTimeout(callback, timeout, args);
  }
}

function emitErrorEvent(request, error) {
  request.emit("error", error);
}

// ClientRequest is imported from node:_http_client (ported from Node.js)

// isCookieField performs a case-insensitive comparison of a provided string
// against the word "cookie." As of V8 6.6 this is faster than handrolling or
// using a case-insensitive RegExp.
function isCookieField(s) {
  return s.length === 6 && s.toLowerCase() === "cookie";
}

function isContentDispositionField(s) {
  return s.length === 19 &&
    s.toLowerCase() === "content-disposition";
}

const kHeaders = Symbol("kHeaders");
const kHeadersDistinct = Symbol("kHeadersDistinct");
const kHeadersCount = Symbol("kHeadersCount");
const kTrailers = Symbol("kTrailers");
const kTrailersDistinct = Symbol("kTrailersDistinct");
const kTrailersCount = Symbol("kTrailersCount");

/** IncomingMessage for http(s) client */
export class IncomingMessageForClient extends NodeReadable {
  decoder = new TextDecoder();

  constructor(socket: Socket) {
    super();

    this._readableState.readingMore = true;

    this.socket = socket;

    this.httpVersionMajor = null;
    this.httpVersionMinor = null;
    this.httpVersion = null;
    this.complete = false;
    this[kHeaders] = null;
    this[kHeadersCount] = 0;
    this.rawHeaders = [];
    this[kTrailers] = null;
    this[kTrailersCount] = 0;
    this.rawTrailers = [];
    this.joinDuplicateHeaders = false;
    this.aborted = false;

    this.upgrade = null;

    // request (server) only
    this.url = "";
    this.method = null;

    // response (client) only
    this.statusCode = null;
    this.statusMessage = null;
    this.client = socket;

    this._consuming = false;
    // Flag for when we decide that this message cannot possibly be
    // read by the user, so there's no point continuing to handle it.
    this._dumped = false;

    this.on("close", () => {
      // Let the final data flush before closing the socket.
      if (this.socket) {
        this.socket.once("drain", () => {
          this.socket.emit("close");
        });
      }
    });
  }

  get connection() {
    return this.socket;
  }

  set connection(val) {
    this.socket = val;
  }

  get headers() {
    if (!this[kHeaders]) {
      this[kHeaders] = {};

      const src = this.rawHeaders;
      const dst = this[kHeaders];

      for (let n = 0; n < this[kHeadersCount]; n += 2) {
        this._addHeaderLine(src[n + 0], src[n + 1], dst);
      }
    }
    return this[kHeaders];
  }

  set headers(val) {
    this[kHeaders] = val;
  }

  get headersDistinct() {
    if (!this[kHeadersDistinct]) {
      this[kHeadersDistinct] = {};

      const src = this.rawHeaders;
      const dst = this[kHeadersDistinct];

      for (let n = 0; n < this[kHeadersCount]; n += 2) {
        this._addHeaderLineDistinct(src[n + 0], src[n + 1], dst);
      }
    }
    return this[kHeadersDistinct];
  }

  set headersDistinct(val) {
    this[kHeadersDistinct] = val;
  }

  get trailers() {
    if (!this[kTrailers]) {
      this[kTrailers] = {};

      const src = this.rawTrailers;
      const dst = this[kTrailers];

      for (let n = 0; n < this[kTrailersCount]; n += 2) {
        this._addHeaderLine(src[n + 0], src[n + 1], dst);
      }
    }
    return this[kTrailers];
  }

  set trailers(val) {
    this[kTrailers] = val;
  }

  get trailersDistinct() {
    if (!this[kTrailersDistinct]) {
      this[kTrailersDistinct] = {};

      const src = this.rawTrailers;
      const dst = this[kTrailersDistinct];

      for (let n = 0; n < this[kTrailersCount]; n += 2) {
        this._addHeaderLineDistinct(src[n + 0], src[n + 1], dst);
      }
    }
    return this[kTrailersDistinct];
  }

  set trailersDistinct(val) {
    this[kTrailersDistinct] = val;
  }

  setTimeout(msecs, callback) {
    if (callback) {
      this.on("timeout", callback);
    }
    this.socket.setTimeout(msecs);
    return this;
  }

  _read(_n) {
    if (!this._consuming) {
      this._readableState.readingMore = false;
      this._consuming = true;
    }

    const buf = new Uint8Array(16 * 1024);

    core.read(this._bodyRid, buf).then(async (bytesRead) => {
      if (bytesRead === 0) {
        // Return the socket to the agent pool BEFORE pushing null.
        // This must happen before the stream ends, otherwise the socket
        // may already be marked as destroyed.
        await this.#tryReturnSocket();
        this.push(null);
      } else {
        this.push(Buffer.from(buf.subarray(0, bytesRead)));
      }
    });
  }

  // Try to return the socket to the agent pool for keepAlive reuse.
  async #tryReturnSocket() {
    const socket = this.socket;
    if (!socket) {
      return;
    }

    const req = this.req;
    // Only pool the socket if keepAlive is enabled.
    if (!req?.shouldKeepAlive) {
      return;
    }

    const handle = req?._socketHandle || socket._handle;
    if (!handle) {
      return;
    }

    try {
      const newRid = await op_node_http_response_reclaim_conn(this._bodyRid);
      if (newRid == null) {
        return;
      }

      const remoteAddr = {
        hostname: socket.remoteAddress || "",
        port: socket.remotePort || 0,
      };
      const localAddr = {
        hostname: socket.localAddress || "",
        port: socket.localPort || 0,
      };

      const isTls = socket.encrypted === true;
      const conn = isTls
        ? new TlsConn(newRid, remoteAddr, localAddr)
        : new TcpConn(newRid, remoteAddr, localAddr);

      handle[kStreamBaseField] = conn;
      if (!socket._handle) {
        socket._handle = handle;
      }

      // Reset socket state for reuse.
      if (socket._readableState?.[kState] !== undefined) {
        socket._readableState[kState] &= ~(kDestroyed | kErrored);
      }
      if (socket._writableState?.[kState] !== undefined) {
        socket._writableState[kState] &=
          ~(kEnding | kEnded | kDestroyed | kErrored);
        socket._writableState.writable = true;
      }
      handle.destroyed = false;

      // Agent's 'free' handler checks socket._httpMessage.shouldKeepAlive.
      if (req && !socket._httpMessage) {
        socket._httpMessage = req;
      }

      // Remove error listener to prevent accumulation on reused sockets.
      if (req?._socketErrorListener) {
        socket.removeListener("error", req._socketErrorListener);
        req._socketErrorListener = null;
      }

      // Mirror Node's emitFreeNT ordering: emit request "close"
      // before socket "free" so userland cleanup (e.g. node-fetch
      // removing per-request socket listeners) runs before the
      // agent reuses the socket.
      if (req) {
        req.destroyed = true;
        req._closed = true;
        req.emit("close");
        req.socket = null;
      }

      // Clear response's socket reference so that when push(null)
      // triggers the response "close" listener, it won't emit
      // "close" on the pooled socket. This must be after req.emit("close")
      // so cleanup handlers can still access the socket via res.socket.
      this.socket = null;

      socket.emit("free");
    } catch (_e) {
      // Socket reuse is best-effort.
    }
  }

  // It's possible that the socket will be destroyed, and removed from
  // any messages, before ever calling this.  In that case, just skip
  // it, since something else is destroying this connection anyway.
  _destroy(err, cb) {
    this.complete = true;
    if (!this.readableEnded || !this.complete) {
      this.aborted = true;
      this.emit("aborted");
    }

    core.tryClose(this._bodyRid);

    // If aborted and the underlying socket is not already destroyed,
    // destroy it.
    // We have to check if the socket is already destroyed because finished
    // does not call the callback when this method is invoked from `_http_client`
    // in `test/parallel/test-http-client-spurious-aborted.js`
    if (this.socket && !this.socket.destroyed && this.aborted) {
      this.socket.destroy(err);
      const cleanup = finished(this.socket, (e) => {
        if (e?.code === "ERR_STREAM_PREMATURE_CLOSE") {
          e = null;
        }
        cleanup();
        onError(this, e || err, cb);
      });
    } else {
      onError(this, err, cb);
    }
  }

  _addHeaderLines(headers, n) {
    if (headers && headers.length) {
      let dest;
      if (this.complete) {
        this.rawTrailers = headers.flat();
        this[kTrailersCount] = n;
        dest = this[kTrailers];
      } else {
        this.rawHeaders = headers.flat();
        this[kHeadersCount] = n;
        dest = this[kHeaders];
      }

      if (dest) {
        for (const header of headers) {
          this._addHeaderLine(header[0], header[1], dest);
        }
      }
    }
  }

  // Add the given (field, value) pair to the message
  //
  // Per RFC2616, section 4.2 it is acceptable to join multiple instances of the
  // same header with a ', ' if the header in question supports specification of
  // multiple values this way. The one exception to this is the Cookie header,
  // which has multiple values joined with a '; ' instead. If a header's values
  // cannot be joined in either of these ways, we declare the first instance the
  // winner and drop the second. Extended header fields (those beginning with
  // 'x-') are always joined.
  _addHeaderLine(field, value, dest) {
    field = matchKnownFields(field);
    const flag = field.charCodeAt(0);
    if (flag === 0 || flag === 2) {
      field = field.slice(1);
      // Make a delimited list
      if (typeof dest[field] === "string") {
        dest[field] += (flag === 0 ? ", " : "; ") + value;
      } else {
        dest[field] = value;
      }
    } else if (flag === 1) {
      // Array header -- only Set-Cookie at the moment
      if (dest["set-cookie"] !== undefined) {
        dest["set-cookie"].push(value);
      } else {
        dest["set-cookie"] = [value];
      }
    } else if (this.joinDuplicateHeaders) {
      // RFC 9110 https://www.rfc-editor.org/rfc/rfc9110#section-5.2
      // https://github.com/nodejs/node/issues/45699
      // allow authorization multiple fields
      // Make a delimited list
      if (dest[field] === undefined) {
        dest[field] = value;
      } else {
        dest[field] += ", " + value;
      }
    } else if (dest[field] === undefined) {
      // Drop duplicates
      dest[field] = value;
    }
  }

  _addHeaderLineDistinct(field, value, dest) {
    field = field.toLowerCase();
    if (!dest[field]) {
      dest[field] = [value];
    } else {
      dest[field].push(value);
    }
  }

  // Call this instead of resume() if we want to just
  // dump all the data to /dev/null
  _dump() {
    if (!this._dumped) {
      this._dumped = true;
      // If there is buffered data, it may trigger 'data' events.
      // Remove 'data' event listeners explicitly.
      this.removeAllListeners("data");
      this.resume();
    }
  }
}

// This function is used to help avoid the lowercasing of a field name if it
// matches a 'traditional cased' version of a field name. It then returns the
// lowercased name to both avoid calling toLowerCase() a second time and to
// indicate whether the field was a 'no duplicates' field. If a field is not a
// 'no duplicates' field, a `0` byte is prepended as a flag. The one exception
// to this is the Set-Cookie header which is indicated by a `1` byte flag, since
// it is an 'array' field and thus is treated differently in _addHeaderLines().
function matchKnownFields(field, lowercased) {
  switch (field.length) {
    case 3:
      if (field === "Age" || field === "age") return "age";
      break;
    case 4:
      if (field === "Host" || field === "host") return "host";
      if (field === "From" || field === "from") return "from";
      if (field === "ETag" || field === "etag") return "etag";
      if (field === "Date" || field === "date") return "\u0000date";
      if (field === "Vary" || field === "vary") return "\u0000vary";
      break;
    case 6:
      if (field === "Server" || field === "server") return "server";
      if (field === "Cookie" || field === "cookie") return "\u0002cookie";
      if (field === "Origin" || field === "origin") return "\u0000origin";
      if (field === "Expect" || field === "expect") return "\u0000expect";
      if (field === "Accept" || field === "accept") return "\u0000accept";
      break;
    case 7:
      if (field === "Referer" || field === "referer") return "referer";
      if (field === "Expires" || field === "expires") return "expires";
      if (field === "Upgrade" || field === "upgrade") return "\u0000upgrade";
      break;
    case 8:
      if (field === "Location" || field === "location") {
        return "location";
      }
      if (field === "If-Match" || field === "if-match") {
        return "\u0000if-match";
      }
      break;
    case 10:
      if (field === "User-Agent" || field === "user-agent") {
        return "user-agent";
      }
      if (field === "Set-Cookie" || field === "set-cookie") {
        return "\u0001";
      }
      if (field === "Connection" || field === "connection") {
        return "\u0000connection";
      }
      break;
    case 11:
      if (field === "Retry-After" || field === "retry-after") {
        return "retry-after";
      }
      break;
    case 12:
      if (field === "Content-Type" || field === "content-type") {
        return "content-type";
      }
      if (field === "Max-Forwards" || field === "max-forwards") {
        return "max-forwards";
      }
      break;
    case 13:
      if (field === "Authorization" || field === "authorization") {
        return "authorization";
      }
      if (field === "Last-Modified" || field === "last-modified") {
        return "last-modified";
      }
      if (field === "Cache-Control" || field === "cache-control") {
        return "\u0000cache-control";
      }
      if (field === "If-None-Match" || field === "if-none-match") {
        return "\u0000if-none-match";
      }
      break;
    case 14:
      if (field === "Content-Length" || field === "content-length") {
        return "content-length";
      }
      break;
    case 15:
      if (field === "Accept-Encoding" || field === "accept-encoding") {
        return "\u0000accept-encoding";
      }
      if (field === "Accept-Language" || field === "accept-language") {
        return "\u0000accept-language";
      }
      if (field === "X-Forwarded-For" || field === "x-forwarded-for") {
        return "\u0000x-forwarded-for";
      }
      break;
    case 16:
      if (field === "Content-Encoding" || field === "content-encoding") {
        return "\u0000content-encoding";
      }
      if (field === "X-Forwarded-Host" || field === "x-forwarded-host") {
        return "\u0000x-forwarded-host";
      }
      break;
    case 17:
      if (field === "If-Modified-Since" || field === "if-modified-since") {
        return "if-modified-since";
      }
      if (field === "Transfer-Encoding" || field === "transfer-encoding") {
        return "\u0000transfer-encoding";
      }
      if (field === "X-Forwarded-Proto" || field === "x-forwarded-proto") {
        return "\u0000x-forwarded-proto";
      }
      break;
    case 19:
      if (field === "Proxy-Authorization" || field === "proxy-authorization") {
        return "proxy-authorization";
      }
      if (field === "If-Unmodified-Since" || field === "if-unmodified-since") {
        return "if-unmodified-since";
      }
      break;
  }
  if (lowercased) {
    return "\u0000" + field;
  }
  return matchKnownFields(field.toLowerCase(), true);
}

function onError(self, error, cb) {
  // This is to keep backward compatible behavior.
  // An error is emitted only if there are listeners attached to the event.
  if (self.listenerCount("error") === 0) {
    cb();
  } else {
    cb(error);
  }
}

export type ServerResponse = {
  req: IncomingMessageForServer;
  statusCode: number;
  statusMessage?: string;

  _headers: Record<string, string | string[]>;
  _hasNonStringHeaders: boolean;

  _readable: ReadableStream;
  finished: boolean;
  headersSent: boolean;
  _resolve: (value: Response | PromiseLike<Response>) => void;
  // deno-lint-ignore no-explicit-any
  _socketOverride: any | null;
  // deno-lint-ignore no-explicit-any
  socket: any | null;

  setHeader(name: string, value: string | string[]): void;
  appendHeader(name: string, value: string | string[]): void;
  getHeader(name: string): string | string[];
  removeHeader(name: string): void;
  getHeaderNames(): string[];
  getHeaders(): Record<string, string | number | string[]>;
  hasHeader(name: string): boolean;

  writeHead(
    status: number,
    statusMessage?: string,
    headers?:
      | Record<string, string | number | string[]>
      | Array<[string, string]>,
  ): void;
  writeHead(
    status: number,
    headers?:
      | Record<string, string | number | string[]>
      | Array<[string, string]>,
  ): void;

  _ensureHeaders(singleChunk?: Chunk): void;

  respond(final: boolean, singleChunk?: Chunk): void;
  // deno-lint-ignore no-explicit-any
  end(chunk?: any, encoding?: any, cb?: any): void;

  flushHeaders(): void;
  writeEarlyHints(
    hints: Record<string, string | string[]>,
    callback?: () => void,
  ): void;
  _implicitHeader(): void;

  // Undocumented field used by `npm:light-my-request`.
  _header: string;

  assignSocket(socket): void;
  detachSocket(socket): void;
} & { -readonly [K in keyof NodeWritable]: NodeWritable[K] };

type ServerResponseStatic = {
  new (
    resolve: (value: Response | PromiseLike<Response>) => void,
    socket: FakeSocket,
  ): ServerResponse;
  _enqueue(controller: ReadableStreamDefaultController, chunk: Chunk): void;
  _bodyShouldBeNull(statusCode: number): boolean;
};

export const ServerResponse = function (
  this: ServerResponse,
  req: IncomingMessageForServer,
  resolve: (value: Response | PromiseLike<Response>) => void,
  socket: FakeSocket,
) {
  this.req = req;
  this.statusCode = 200;
  this.statusMessage = undefined;
  this._headers = { __proto__: null };
  this._hasNonStringHeaders = false;
  this.writable = true;

  // used by `npm:on-finished`
  this.finished = false;
  this.headersSent = false;
  this._socketOverride = null;

  let controller: ReadableByteStreamController;
  const readable = new ReadableStream({
    start(c) {
      controller = c as ReadableByteStreamController;
    },
  });

  NodeWritable.call(
    this,
    {
      autoDestroy: true,
      defaultEncoding: "utf-8",
      emitClose: true,
      // FIXME: writes don't work when a socket is assigned and then
      // detached.
      write: (chunk, encoding, cb) => {
        // Writes chunks are directly written to the socket if
        // one is assigned via assignSocket()
        if (this._socketOverride && this._socketOverride.writable) {
          this._socketOverride.write(chunk, encoding);
          return cb();
        }
        if (!this.headersSent) {
          ServerResponse._enqueue(controller, chunk);
          this.respond(false);
          return cb();
        }
        ServerResponse._enqueue(controller, chunk);
        return cb();
      },
      final: (cb) => {
        if (!this.headersSent) {
          this.respond(true);
        }
        controller.close();
        return cb();
      },
      destroy: (err, cb) => {
        if (err) {
          controller.error(err);
        }
        return cb(null);
      },
    } satisfies NodeWritableOptions,
  );

  this._readable = readable;
  this._resolve = resolve;
  this.socket = socket;
  this.on("newListener", (event) => {
    if (event === "close") {
      this.socket?.[kBindToAbortSignal]();
      this.socket?.on("close", () => {
        if (!this.finished) {
          this.emit("close");
        }
      });
    }
  });
  this._header = "";
} as unknown as ServerResponseStatic;

Object.setPrototypeOf(ServerResponse.prototype, NodeWritable.prototype);
Object.setPrototypeOf(ServerResponse, NodeWritable);

ServerResponse._enqueue = function (
  this: ServerResponse,
  controller: ReadableStreamDefaultController,
  chunk: Chunk,
) {
  try {
    if (typeof chunk === "string") {
      controller.enqueue(ENCODER.encode(chunk));
    } else {
      controller.enqueue(chunk);
    }
  } catch (_) {
    // The stream might have been closed. Ignore the error.
  }
};

/** Returns true if the response body should be null with the given
 * http status code */
ServerResponse._bodyShouldBeNull = function (
  this: ServerResponse,
  status: number,
) {
  return status === 101 || status === 204 || status === 205 || status === 304;
};

ServerResponse.prototype.setHeader = function (
  this: ServerResponse,
  name: string,
  value: string | string[],
) {
  if (Array.isArray(value)) {
    this._hasNonStringHeaders = true;
  }
  this._headers[StringPrototypeToLowerCase(name)] = value;
  return this;
};

ServerResponse.prototype.setHeaders = function setHeaders(
  this: ServerResponse,
  headers: Headers | Map<string, string | string[]>,
) {
  if (this._header) {
    throw new ERR_HTTP_HEADERS_SENT("set");
  }

  if (
    !headers ||
    ArrayIsArray(headers) ||
    typeof headers.keys !== "function" ||
    typeof headers.get !== "function"
  ) {
    throw new ERR_INVALID_ARG_TYPE("headers", ["Headers", "Map"], headers);
  }

  // Headers object joins multiple cookies with a comma when using
  // the getter to retrieve the value,
  // unless iterating over the headers directly.
  // We also cannot safely split by comma.
  // To avoid setHeader overwriting the previous value we push
  // set-cookie values in array and set them all at once.
  const cookies = [];

  for (const { 0: key, 1: value } of headers) {
    if (key === "set-cookie") {
      if (ArrayIsArray(value)) {
        cookies.push(...value);
      } else {
        cookies.push(value);
      }
      continue;
    }
    this.setHeader(key, value);
  }
  if (cookies.length) {
    this.setHeader("set-cookie", cookies);
  }

  return this;
};

ServerResponse.prototype.appendHeader = function (
  this: ServerResponse,
  name: string,
  value: string | string[],
) {
  const key = StringPrototypeToLowerCase(name);
  if (this._headers[key] === undefined) {
    if (Array.isArray(value)) this._hasNonStringHeaders = true;
    this._headers[key] = value;
  } else {
    this._hasNonStringHeaders = true;
    if (!Array.isArray(this._headers[key])) {
      this._headers[key] = [this._headers[key]];
    }
    const header = this._headers[key];
    if (Array.isArray(value)) {
      header.push(...value);
    } else {
      header.push(value);
    }
  }
  return this;
};

ServerResponse.prototype.getHeader = function (
  this: ServerResponse,
  name: string,
) {
  return this._headers[StringPrototypeToLowerCase(name)];
};

ServerResponse.prototype.removeHeader = function (
  this: ServerResponse,
  name: string,
) {
  delete this._headers[StringPrototypeToLowerCase(name)];
};

ServerResponse.prototype.getHeaderNames = function (this: ServerResponse) {
  return Object.keys(this._headers);
};

ServerResponse.prototype.getHeaders = function (
  this: ServerResponse,
): Record<string, string | number | string[]> {
  return { __proto__: null, ...this._headers };
};

ServerResponse.prototype.hasHeader = function (
  this: ServerResponse,
  name: string,
) {
  return Object.hasOwn(this._headers, StringPrototypeToLowerCase(name));
};

ServerResponse.prototype.writeHead = function (
  this: ServerResponse,
  status: number,
  statusMessageOrHeaders?:
    | string
    | Record<string, string | number | string[]>
    | Array<[string, string]>
    | Array<string>,
  maybeHeaders?:
    | Record<string, string | number | string[]>
    | Array<[string, string]>
    | Array<string>,
) {
  this.statusCode = status;

  let headers = null;
  if (typeof statusMessageOrHeaders === "string") {
    this.statusMessage = statusMessageOrHeaders;
    if (maybeHeaders !== undefined) {
      headers = maybeHeaders;
    }
  } else if (statusMessageOrHeaders !== undefined) {
    headers = statusMessageOrHeaders;
  }

  if (headers !== null) {
    if (ArrayIsArray(headers)) {
      headers = headers as Array<[string, string]> | Array<string>;

      // Headers should override previous headers but still
      // allow explicit duplicates. To do so, we first remove any
      // existing conflicts, then use appendHeader.

      if (ArrayIsArray(headers[0])) {
        headers = headers as Array<[string, string]>;
        for (let i = 0; i < headers.length; i++) {
          const headerTuple = headers[i];
          const k = headerTuple[0];
          if (k) this.removeHeader(k);
        }

        for (let i = 0; i < headers.length; i++) {
          const headerTuple = headers[i];
          const k = headerTuple[0];
          if (k) this.appendHeader(k, headerTuple[1]);
        }
      } else {
        headers = headers as Array<string>;
        for (let i = 0; i < headers.length; i += 2) {
          const k = headers[i];
          this.removeHeader(k);
        }

        for (let i = 0; i < headers.length; i += 2) {
          const k = headers[i];
          if (k) this.appendHeader(k, headers[i + 1]);
        }
      }
    } else {
      headers = headers as Record<string, string>;
      for (const k in headers) {
        if (Object.hasOwn(headers, k)) {
          this.setHeader(k, headers[k]);
        }
      }
    }
  }

  return this;
};

ServerResponse.prototype._ensureHeaders = function (
  this: ServerResponse,
  singleChunk?: Chunk,
) {
  if (this.statusCode === 200 && this.statusMessage === undefined) {
    this.statusMessage = "OK";
  }
  if (typeof singleChunk === "string" && !this.hasHeader("content-type")) {
    this.setHeader("content-type", "text/plain;charset=UTF-8");
  }
};

ServerResponse.prototype.respond = function (
  this: ServerResponse,
  final: boolean,
  singleChunk?: Chunk,
) {
  this.headersSent = true;
  this._ensureHeaders(singleChunk);
  let body = singleChunk ?? (final ? null : this._readable);
  if (ServerResponse._bodyShouldBeNull(this.statusCode)) {
    body = null;
  }
  let headers: Record<string, string> | [string, string][] = this
    ._headers as Record<string, string>;
  if (this._hasNonStringHeaders) {
    headers = [];
    // Guard is not needed as this is a null prototype object.
    // deno-lint-ignore guard-for-in
    for (const key in this._headers) {
      const entry = this._headers[key];
      if (Array.isArray(entry)) {
        for (const value of entry) {
          headers.push([key, value]);
        }
      } else {
        headers.push([key, entry]);
      }
    }
  }
  this._resolve(
    new Response(body, {
      headers,
      status: this.statusCode,
      statusText: this.statusMessage,
    }),
  );
};

ServerResponse.prototype.end = function (
  this: ServerResponse,
  // deno-lint-ignore no-explicit-any
  chunk?: any,
  // deno-lint-ignore no-explicit-any
  encoding?: any,
  // deno-lint-ignore no-explicit-any
  cb?: any,
) {
  this.finished = true;
  if (!chunk && "transfer-encoding" in this._headers) {
    // FIXME(bnoordhuis) Node sends a zero length chunked body instead, i.e.,
    // the trailing "0\r\n", but respondWith() just hangs when I try that.
    this._headers["content-length"] = "0";
    delete this._headers["transfer-encoding"];
  }

  // @ts-expect-error The signature for cb is stricter than the one implemented here
  NodeWritable.prototype.end.call(this, chunk, encoding, cb);
};

ServerResponse.prototype.flushHeaders = function (this: ServerResponse) {
  // no-op
};

// Undocumented API used by `npm:compression`.
ServerResponse.prototype._implicitHeader = function (this: ServerResponse) {
  this.writeHead(this.statusCode);
};

ServerResponse.prototype.assignSocket = function (
  this: ServerResponse,
  socket,
) {
  if (socket._httpMessage) {
    throw new ERR_HTTP_SOCKET_ASSIGNED();
  }
  socket._httpMessage = this;
  this._socketOverride = socket;
};

ServerResponse.prototype.detachSocket = function (
  this: ServerResponse,
  socket,
) {
  assert(socket._httpMessage === this);
  socket._httpMessage = null;
  this._socketOverride = null;
};

ServerResponse.prototype.writeContinue = function writeContinue(cb) {
  if (cb) {
    nextTick(cb);
  }
};

ServerResponse.prototype.writeEarlyHints = function writeEarlyHints(
  _hints,
  cb,
) {
  if (cb) {
    nextTick(cb);
  }
};

Object.defineProperty(ServerResponse.prototype, "connection", {
  get: deprecate(
    function (this: ServerResponse) {
      return this._socketOverride;
    },
    "ServerResponse.prototype.connection is deprecated",
    "DEP0066",
  ),
  set: deprecate(
    // deno-lint-ignore no-explicit-any
    function (this: ServerResponse, socket: any) {
      this._socketOverride = socket;
    },
    "ServerResponse.prototype.connection is deprecated",
    "DEP0066",
  ),
});

const kRawHeaders = Symbol("rawHeaders");

// TODO(@AaronO): optimize
export class IncomingMessageForServer extends NodeReadable {
  #headers: Record<string, string>;
  url: string;
  method: string;
  socket: Socket | FakeSocket;

  constructor(socket: FakeSocket | Socket) {
    const reader = socket instanceof FakeSocket
      ? socket.reader
      : socket instanceof Socket
      ? NodeDuplex.toWeb(socket).readable.getReader()
      : null;
    super({
      autoDestroy: true,
      emitClose: true,
      objectMode: false,
      read: async function (_size) {
        if (!reader) {
          return this.push(null);
        }

        try {
          const { value } = await reader!.read();
          this.push(value !== undefined ? Buffer.from(value) : null);
        } catch (err) {
          this.destroy(err as Error);
        }
      },
      destroy: (err, cb) => {
        reader?.cancel().catch(() => {
          // Don't throw error - it's propagated to the user via 'error' event.
        }).finally(nextTick(onError, this, err, cb));
      },
    });
    this.url = "";
    this.method = "";
    this.socket = socket;
    this.upgrade = null;
    this[kRawHeaders] = [];
    socket?.on("error", (e) => {
      if (this.listenerCount("error") > 0) {
        this.emit("error", e);
      }
    });
  }

  get aborted() {
    return false;
  }

  get httpVersion() {
    return "1.1";
  }

  set httpVersion(val) {
    assert(val === "1.1");
  }

  get headers() {
    if (!this.#headers) {
      this.#headers = {};
      const entries = headersEntries(this[kRawHeaders]);
      for (let i = 0; i < entries.length; i++) {
        const entry = entries[i];
        this.#headers[entry[0]] = entry[1];
      }
    }
    return this.#headers;
  }

  set headers(val) {
    this.#headers = val;
  }

  get rawHeaders() {
    const entries = headersEntries(this[kRawHeaders]);
    const out = new Array(entries.length * 2);
    for (let i = 0; i < entries.length; i++) {
      out[i * 2] = entries[i][0];
      out[i * 2 + 1] = entries[i][1];
    }
    return out;
  }

  // connection is deprecated, but still tested in unit test.
  get connection() {
    return this.socket;
  }

  setTimeout(msecs, callback) {
    if (callback) {
      this.on("timeout", callback);
    }
    this.socket.setTimeout(msecs);
    return this;
  }
}

export type ServerHandler = (
  req: IncomingMessageForServer,
  res: ServerResponse,
) => void;

export function Server(opts, requestListener?: ServerHandler): ServerImpl {
  return new ServerImpl(opts, requestListener);
}

function _addAbortSignalOption(server: ServerImpl, options: ListenOptions) {
  if (options?.signal === undefined) {
    return;
  }

  validateAbortSignal(options.signal, "options.signal");
  const { signal } = options;

  const onAborted = () => {
    server.close();
  };

  if (signal.aborted) {
    nextTick(onAborted);
  } else {
    signal.addEventListener("abort", onAborted);
    server.once("close", () => signal.removeEventListener("abort", onAborted));
  }
}

export class ServerImpl extends EventEmitter {
  #addr: Deno.NetAddr | null = null;
  #hasClosed = false;
  #server: Deno.HttpServer;
  #unref = false;
  #ac?: AbortController;
  #listener: Deno.Listener | null = null;
  #serveDeferred: ReturnType<typeof Promise.withResolvers<void>>;
  listening = false;

  constructor(opts, requestListener?: ServerHandler) {
    super();

    if (typeof opts === "function") {
      requestListener = opts;
      opts = kEmptyObject;
    } else if (opts == null) {
      opts = kEmptyObject;
    } else {
      validateObject(opts, "options");
    }

    this._opts = opts;

    this.#serveDeferred = Promise.withResolvers<void>();
    this.#serveDeferred.promise.then(() => this.emit("close"));
    if (requestListener !== undefined) {
      this.on("request", requestListener);
    }
  }

  listen(...args: unknown[]): this {
    // TODO(bnoordhuis) Delegate to net.Server#listen().
    const normalized = _normalizeArgs(args);
    const options = normalized[0] as Partial<ListenOptions>;
    const cb = normalized[1];

    if (cb !== null) {
      // @ts-ignore change EventEmitter's sig to use CallableFunction
      this.once("listening", cb);
    }

    let port = 0;
    if (typeof options.port === "number" || typeof options.port === "string") {
      validatePort(options.port, "options.port");
      port = options.port | 0;
    }

    _addAbortSignalOption(this, options);

    // TODO(bnoordhuis) Node prefers [::] when host is omitted,
    // we on the other hand default to 0.0.0.0.
    let hostname = options.host ?? "0.0.0.0";
    if (hostname == "localhost") {
      hostname = "127.0.0.1";
    }

    // Check DENO_SERVE_ADDRESS override (used by desktop runtime, Deno Deploy, etc.)
    if (!nodeHttpAddressOverrideConsumed) {
      const {
        0: overrideKind,
        1: overrideHost,
        2: overridePort,
      } = op_http_serve_address_override();
      if (overrideKind === 1) {
        // TCP override
        nodeHttpAddressOverrideConsumed = true;
        hostname = overrideHost;
        port = overridePort;
      }
    }

    // Bind the port synchronously so that address() returns the actual
    // port immediately after listen(), matching Node.js behavior.
    try {
      this.#listener = this._listen(hostname, port);
    } catch (e) {
      // Emit the error asynchronously, matching Node.js behavior.
      this.#addr = { hostname, port } as Deno.NetAddr;
      nextTick(() => this.emit("error", e));
      return this;
    }
    const addr = this.#listener.addr as Deno.NetAddr;
    this.#addr = {
      hostname: addr.hostname,
      port: addr.port,
    } as Deno.NetAddr;
    this.listening = true;
    nextTick(() => this._serve());

    return this;
  }

  _listen(hostname: string, port: number): Deno.Listener {
    return listenDeno({ hostname, port });
  }

  _serve() {
    const ac = new AbortController();
    const handler = (request: Request, info: Deno.ServeHandlerInfo) => {
      const socket = new FakeSocket({
        remoteAddress: info.remoteAddr.hostname,
        remotePort: info.remoteAddr.port,
        encrypted: this._encrypted,
        reader: request.body?.getReader(),
        request,
      });

      const req = new IncomingMessageForServer(socket);
      req.method = request.method;

      if (request.method === "CONNECT") {
        // For CONNECT, the URL should be in authority form (host:port).
        // Deno's server adds an "http://" prefix, so strip it.
        req.url = request.url.replace(/^https?:\/\//, "");
        req[kRawHeaders] = request.headers;

        if (this.listenerCount("connect") > 0) {
          return (async () => {
            const { conn, response, head } = await upgradeHttpRawConnect(
              request,
            );
            const socket = new Socket({
              handle: new TCP(constants.SERVER, conn),
            });
            req.socket = socket;
            this.emit("connect", req, socket, Buffer.from(head));
            return response;
          })();
        } else {
          return new Response(null, { status: 405 });
        }
      }

      // Slice off the origin so that we only have pathname + search
      req.url = request.url?.slice(request.url.indexOf("/", 8));
      req.upgrade =
        request.headers.get("connection")?.toLowerCase().includes("upgrade") &&
        request.headers.get("upgrade");
      req[kRawHeaders] = request.headers;

      // Don't fire the "upgrade" event for h2c (HTTP/2 cleartext) upgrades.
      // These are protocol-level upgrades that aren't meant for user-space
      // handlers (like WebSocket). Treating them as regular requests lets
      // the server respond normally with HTTP/1.1.
      if (
        req.upgrade && req.upgrade.toLowerCase() !== "h2c" &&
        this.listenerCount("upgrade") > 0
      ) {
        const { conn, response } = upgradeHttpRaw(request);
        const socket = new Socket({
          handle: new TCP(constants.SERVER, conn),
        });
        // Update socket held by `req`.
        req.socket = socket;
        this.emit("upgrade", req, socket, Buffer.from([]));
        return response;
      } else {
        return new Promise<Response>((resolve): void => {
          const res = new ServerResponse(req, resolve, socket);

          if (request.headers.has("expect")) {
            if (/(?:^|\W)100-continue(?:$|\W)/i.test(req.headers.expect)) {
              if (this.listenerCount("checkContinue") > 0) {
                this.emit("checkContinue", req, res);
              } else {
                res.writeContinue();
                this.emit("request", req, res);
              }
            } else if (this.listenerCount("checkExpectation") > 0) {
              this.emit("checkExpectation", req, res);
            } else {
              res.writeHead(417);
              res.end();
            }
          } else {
            this.emit("request", req, res);
          }
        });
      }
    };

    if (this.#hasClosed) {
      return;
    }
    this.#ac = ac;
    const listener = this.#listener;
    this.#listener = null;
    if (!listener) {
      return;
    }
    try {
      this.#server = serveHttpOnListener(
        listener,
        ac.signal,
        handler,
        (_error) => {
          return new Response("Internal Server Error", { status: 500 });
        },
        () => {
          this.emit("listening");
        },
      );
    } catch (e) {
      this.emit("error", e);
      return;
    }

    if (this.#unref) {
      this.#server.unref();
    }
    this.#server.finished.then(() => this.#serveDeferred!.resolve());
  }

  setTimeout() {
    // deno-lint-ignore no-console
    console.error("Not implemented: Server.setTimeout()");
  }

  ref() {
    if (this.#server) {
      this.#server.ref();
    }
    this.#unref = false;

    return this;
  }

  unref() {
    if (this.#server) {
      this.#server.unref();
    }
    this.#unref = true;

    return this;
  }

  close(cb?: (err?: Error) => void): this {
    const listening = this.listening;
    this.listening = false;

    this.#hasClosed = true;
    if (typeof cb === "function") {
      if (listening) {
        this.once("close", cb);
      } else {
        this.once("close", function close() {
          cb(new ERR_SERVER_NOT_RUNNING());
        });
      }
    }

    // Close pre-bound listener if _serve() hasn't consumed it yet.
    if (this.#listener) {
      this.#listener.close();
      this.#listener = null;
    }

    if (listening && this.#ac) {
      if (this.#server) {
        this.#server.shutdown();
      } else if (this.#ac) {
        this.#ac.abort();
        this.#ac = undefined;
      }
    } else {
      this.#serveDeferred!.resolve();
    }

    this.#server = undefined;
    return this;
  }

  closeAllConnections() {
    if (this.#hasClosed) {
      return;
    }
    if (this.#ac) {
      this.#ac.abort();
      this.#ac = undefined;
    }
  }

  closeIdleConnections() {
    if (this.#hasClosed) {
      return;
    }

    if (this.#server) {
      this.#server.shutdown();
    }
  }

  address() {
    if (this.#addr === null) return null;
    const addr = this.#addr.hostname;
    // Match Node.js: family is undefined for non-IP addresses (isIP returns 0)
    const family = getIPFamily(addr);
    return { port: this.#addr.port, address: addr, family };
  }
}

Server.prototype = ServerImpl.prototype;

export function createServer(opts, requestListener?: ServerHandler) {
  return Server(opts, requestListener);
}

/** Makes an HTTP request. */
export function request(
  url: string | URL,
  cb?: (res: IncomingMessageForClient) => void,
): ClientRequest;
export function request(
  opts: RequestOptions,
  cb?: (res: IncomingMessageForClient) => void,
): ClientRequest;
export function request(
  url: string | URL,
  opts: RequestOptions,
  cb?: (res: IncomingMessageForClient) => void,
): ClientRequest;
// deno-lint-ignore no-explicit-any
export function request(...args: any[]) {
  return new ClientRequest(args[0], args[1], args[2]);
}

/** Makes a `GET` HTTP request. */
export function get(
  url: string | URL,
  cb?: (res: IncomingMessageForClient) => void,
): ClientRequest;
export function get(
  opts: RequestOptions,
  cb?: (res: IncomingMessageForClient) => void,
): ClientRequest;
export function get(
  url: string | URL,
  opts: RequestOptions,
  cb?: (res: IncomingMessageForClient) => void,
): ClientRequest;
// deno-lint-ignore no-explicit-any
export function get(...args: any[]) {
  const req = request(args[0], args[1], args[2]);
  req.end();
  return req;
}

export const maxHeaderSize = 16_384;

export {
  Agent,
  ClientRequest,
  globalAgent,
  IncomingMessageForServer as IncomingMessage,
  METHODS,
  OutgoingMessage,
  STATUS_CODES,
  validateHeaderName,
  validateHeaderValue,
};
export default {
  Agent,
  globalAgent,
  ClientRequest,
  STATUS_CODES,
  METHODS,
  createServer,
  Server,
  IncomingMessage: IncomingMessageForServer,
  IncomingMessageForClient,
  IncomingMessageForServer,
  OutgoingMessage,
  ServerResponse,
  request,
  get,
  validateHeaderName,
  validateHeaderValue,
  maxHeaderSize,
};
