// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core } from "ext:core/mod.js";
import { op_node_http_response_reclaim_conn } from "ext:core/ops";

import { Buffer } from "node:buffer";
import { Duplex as NodeDuplex, Readable as NodeReadable } from "node:stream";
import {
  OutgoingMessage,
  validateHeaderName,
  validateHeaderValue,
} from "node:_http_outgoing";
import { ClientRequest } from "node:_http_client";
import { Agent, globalAgent } from "node:_http_agent";
import { kStreamBaseField } from "ext:deno_node/internal_binding/stream_wrap.ts";
import {
  kDestroyed,
  kEnded,
  kEnding,
  kErrored,
  kState,
} from "ext:deno_node/internal/streams/utils.js";
import { TcpConn } from "ext:deno_net/01_net.js";
import { TlsConn } from "ext:deno_net/02_tls.js";
import {
  Server as ServerImpl,
  ServerResponse,
  STATUS_CODES,
} from "node:_http_server";
import { methods as METHODS } from "node:_http_common";

type Chunk = string | Buffer | Uint8Array;

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

function _validateHost(host, name) {
  if (host !== null && host !== undefined && typeof host !== "string") {
    throw new ERR_INVALID_ARG_TYPE(`options.${name}`, [
      "string",
      "undefined",
      "null",
    ], host);
  }
  return host;
}

const _INVALID_PATH_REGEX = /[^\u0021-\u00ff]/;
const _kError = Symbol("kError");

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

// deno-lint-ignore no-explicit-any
export type ServerHandler = (req: any, res: any) => void;

export function createServer(opts, requestListener?: ServerHandler) {
  return new ServerImpl(opts, requestListener);
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
  ServerImpl,
  ServerImpl as Server,
  ServerResponse,
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
  Server: ServerImpl,
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
