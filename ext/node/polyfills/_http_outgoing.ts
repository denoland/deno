// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

const core = globalThis.__bootstrap.core;
import { getDefaultHighWaterMark } from "ext:deno_node/internal/streams/state.mjs";
import assert from "ext:deno_node/internal/assert.mjs";
import EE from "ext:deno_node/events.ts";
import { Stream } from "ext:deno_node/stream.ts";
import { deprecate } from "ext:deno_node/util.ts";
import type { Socket } from "ext:deno_node/net.ts";
import {
  kNeedDrain,
  kOutHeaders,
  // utcDate,
} from "ext:deno_node/internal/http.ts";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
import {
  _checkInvalidHeaderChar as checkInvalidHeaderChar,
  _checkIsHttpToken as checkIsHttpToken,
  // chunkExpression as RE_TE_CHUNKED,
} from "ext:deno_node/_http_common.ts";
import {
  defaultTriggerAsyncIdScope,
  symbols,
} from "ext:deno_node/internal/async_hooks.ts";
// deno-lint-ignore camelcase
const { async_id_symbol } = symbols;
import {
  ERR_HTTP_HEADERS_SENT,
  ERR_HTTP_INVALID_HEADER_VALUE,
  // ERR_HTTP_TRAILER_INVALID,
  // ERR_INVALID_ARG_TYPE,
  // ERR_INVALID_ARG_VALUE,
  ERR_INVALID_CHAR,
  ERR_INVALID_HTTP_TOKEN,
  ERR_METHOD_NOT_IMPLEMENTED,
  // ERR_STREAM_ALREADY_FINISHED,
  ERR_STREAM_CANNOT_PIPE,
  // ERR_STREAM_DESTROYED,
  // ERR_STREAM_NULL_VALUES,
  // ERR_STREAM_WRITE_AFTER_END,
  hideStackFrames,
} from "ext:deno_node/internal/errors.ts";
import { validateString } from "ext:deno_node/internal/validators.mjs";
// import { isUint8Array } from "ext:deno_node/internal/util/types.ts";
// import { kStreamBaseField } from "ext:deno_node/internal_binding/stream_wrap.ts";

import { debuglog } from "ext:deno_node/internal/util/debuglog.ts";
let debug = debuglog("http", (fn) => {
  debug = fn;
});

const HIGH_WATER_MARK = getDefaultHighWaterMark();

const kCorked = Symbol("corked");

const nop = () => {};

export class OutgoingMessage extends Stream {
  // deno-lint-ignore no-explicit-any
  outputData: any[];
  outputSize: number;
  writable: boolean;
  destroyed: boolean;

  _last: boolean;
  chunkedEncoding: boolean;
  shouldKeepAlive: boolean;
  maxRequestsOnConnectionReached: boolean;
  _defaultKeepAlive: boolean;
  useChunkedEncodingByDefault: boolean;
  sendDate: boolean;
  _removedConnection: boolean;
  _removedContLen: boolean;
  _removedTE: boolean;

  _contentLength: number | null;
  _hasBody: boolean;
  _trailer: string;
  [kNeedDrain]: boolean;

  finished: boolean;
  _headerSent: boolean;
  [kCorked]: number;
  _closed: boolean;

  // TODO(crowlKats): use it
  socket: null;
  // TODO(crowlKats): use it
  _header: null;
  [kOutHeaders]: null | Record<string, [string, string]>;

  _keepAliveTimeout: number;
  _onPendingData: () => void;

  constructor() {
    super();

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

    this.socket = null;
    this._header = null;
    this[kOutHeaders] = null;

    this._keepAliveTimeout = 0;

    this._onPendingData = nop;
  }

  get writableFinished() {
    return (
      this.finished &&
      this.outputSize === 0 &&
      (!this.socket || this.socket.writableLength === 0)
    );
  }

  get writableObjectMode() {
    return false;
  }

  get writableLength() {
    return this.outputSize + (this.socket ? this.socket.writableLength : 0);
  }

  get writableHighWaterMark() {
    return this.socket ? this.socket.writableHighWaterMark : HIGH_WATER_MARK;
  }

  get writableCorked() {
    const corked = this.socket ? this.socket.writableCorked : 0;
    return corked + this[kCorked];
  }

  get connection() {
    return this.socket;
  }

  set connection(val) {
    this.socket = val;
  }

  get writableEnded() {
    return this.finished;
  }

  get writableNeedDrain() {
    return !this.destroyed && !this.finished && this[kNeedDrain];
  }

  cork() {
    if (this.socket) {
      this.socket.cork();
    } else {
      this[kCorked]++;
    }
  }

  uncork() {
    if (this.socket) {
      this.socket.uncork();
    } else if (this[kCorked]) {
      this[kCorked]--;
    }
  }

  setTimeout(msecs: number, callback?: (...args: unknown[]) => void) {
    if (callback) {
      this.on("timeout", callback);
    }

    if (!this.socket) {
      // deno-lint-ignore no-explicit-any
      this.once("socket", function socketSetTimeoutOnConnect(socket: any) {
        socket.setTimeout(msecs);
      });
    } else {
      this.socket.setTimeout(msecs);
    }
    return this;
  }

  // It's possible that the socket will be destroyed, and removed from
  // any messages, before ever calling this.  In that case, just skip
  // it, since something else is destroying this connection anyway.
  destroy(error: unknown) {
    if (this.destroyed) {
      return this;
    }
    this.destroyed = true;

    if (this.socket) {
      this.socket.destroy(error);
    } else {
      // deno-lint-ignore no-explicit-any
      this.once("socket", function socketDestroyOnConnect(socket: any) {
        socket.destroy(error);
      });
    }

    return this;
  }

  setHeader(name: string, value: string) {
    if (this._header) {
      throw new ERR_HTTP_HEADERS_SENT("set");
    }
    validateHeaderName(name);
    validateHeaderValue(name, value);

    let headers = this[kOutHeaders];
    if (headers === null) {
      this[kOutHeaders] = headers = Object.create(null);
    }

    name = name.toString();
    headers[name.toLowerCase()] = [name, value.toString()];
    return this;
  }

  appendHeader(name, value) {
    if (this._header) {
      throw new ERR_HTTP_HEADERS_SENT("append");
    }
    validateHeaderName(name);
    validateHeaderValue(name, value);

    name = name.toString();

    const field = name.toLowerCase();
    const headers = this[kOutHeaders];
    if (headers === null || !headers[field]) {
      return this.setHeader(name, value);
    }

    // Prepare the field for appending, if required
    if (!Array.isArray(headers[field][1])) {
      headers[field][1] = [headers[field][1]];
    }

    const existingValues = headers[field][1];
    if (Array.isArray(value)) {
      for (let i = 0, length = value.length; i < length; i++) {
        existingValues.push(value[i].toString());
      }
    } else {
      existingValues.push(value.toString());
    }

    return this;
  }

  // Returns a shallow copy of the current outgoing headers.
  getHeaders() {
    const headers = this[kOutHeaders];
    const ret = Object.create(null);
    if (headers) {
      const keys = Object.keys(headers);
      // Retain for(;;) loop for performance reasons
      // Refs: https://github.com/nodejs/node/pull/30958
      for (let i = 0; i < keys.length; ++i) {
        const key = keys[i];
        const val = headers[key][1];
        ret[key] = val;
      }
    }
    return ret;
  }

  hasHeader(name: string) {
    validateString(name, "name");
    return this[kOutHeaders] !== null &&
      !!this[kOutHeaders][name.toLowerCase()];
  }

  removeHeader(name: string) {
    validateString(name, "name");

    if (this._header) {
      throw new ERR_HTTP_HEADERS_SENT("remove");
    }

    const key = name.toLowerCase();

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
  }

  getHeader(name: string) {
    validateString(name, "name");

    const headers = this[kOutHeaders];
    if (headers === null) {
      return;
    }

    const entry = headers[name.toLowerCase()];
    return entry && entry[1];
  }

  // Returns an array of the names of the current outgoing headers.
  getHeaderNames() {
    return this[kOutHeaders] !== null ? Object.keys(this[kOutHeaders]) : [];
  }

  // Returns an array of the names of the current outgoing raw headers.
  getRawHeaderNames() {
    const headersMap = this[kOutHeaders];
    if (headersMap === null) return [];

    const values = Object.values(headersMap);
    const headers = Array(values.length);
    // Retain for(;;) loop for performance reasons
    // Refs: https://github.com/nodejs/node/pull/30958
    for (let i = 0, l = values.length; i < l; i++) {
      // deno-lint-ignore no-explicit-any
      headers[i] = (values as any)[i][0];
    }

    return headers;
  }

  write(
    chunk: string | Uint8Array | Buffer,
    encoding: string | null,
    callback: () => void,
  ): boolean {
    if (
      (typeof chunk === "string" && chunk.length > 0) ||
      ((chunk instanceof Buffer || chunk instanceof Uint8Array) &&
        chunk.buffer.byteLength > 0)
    ) {
      if (typeof chunk === "string") {
        chunk = Buffer.from(chunk, encoding);
      }
      if (chunk instanceof Buffer) {
        chunk = new Uint8Array(chunk.buffer);
      }

      core.writeAll(this._bodyWriteRid, chunk).then(() => {
        callback();
        this.emit("drain");
      }).catch((e) => {
        this._requestSendErrorSet = e;
      });
    }

    return false;
  }

  // deno-lint-ignore no-explicit-any
  addTrailers(_headers: any) {
    // TODO(crowlKats): finish it
    notImplemented("OutgoingMessage.addTrailers");
  }

  // deno-lint-ignore no-explicit-any
  end(_chunk: any, _encoding: any, _callback: any) {
    notImplemented("OutgoingMessage.end");
  }

  flushHeaders() {
    if (!this._header) {
      this._implicitHeader();
    }

    // Force-flush the headers.
    this._send("");
  }

  pipe() {
    // OutgoingMessage should be write-only. Piping from it is disabled.
    this.emit("error", new ERR_STREAM_CANNOT_PIPE());
  }

  _implicitHeader() {
    throw new ERR_METHOD_NOT_IMPLEMENTED("_implicitHeader()");
  }

  _finish() {
    assert(this.socket);
    this.emit("prefinish");
  }

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
  }

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
    }
    socket.uncork();

    this.outputData = [];
    this._onPendingData(-this.outputSize);
    this.outputSize = 0;

    return ret;
  }

  // This abstract either writing directly to the socket or buffering it.
  // deno-lint-ignore no-explicit-any
  _send(data: any, encoding?: string | null, callback?: () => void) {
    // This is a shameful hack to get the headers and first body chunk onto
    // the same packet. Future versions of Node are going to take care of
    // this at a lower level and in a more general way.
    if (!this._headerSent && this._header !== null) {
      // `this._header` can be null if OutgoingMessage is used without a proper Socket
      // See: /test/parallel/test-http-outgoing-message-inheritance.js
      if (
        typeof data === "string" &&
        (encoding === "utf8" || encoding === "latin1" || !encoding)
      ) {
        data = this._header + data;
      } else {
        const header = this._header;
        this.outputData.unshift({
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
  }

  _writeRaw(
    // deno-lint-ignore no-explicit-any
    this: any,
    // deno-lint-ignore no-explicit-any
    data: any,
    encoding?: string | null,
    callback?: () => void,
  ) {
    const conn = this.socket;
    if (conn && conn.destroyed) {
      // The socket was destroyed. If we're still trying to write to it,
      // then we haven't gotten the 'close' event yet.
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
      // Directly write to socket.
      return conn.write(data, encoding, callback);
    }
    // Buffer, as long as we're not destroyed.
    this.outputData.push({ data, encoding, callback });
    this.outputSize += data.length;
    this._onPendingData(data.length);
    return this.outputSize < HIGH_WATER_MARK;
  }

  _renderHeaders() {
    if (this._header) {
      throw new ERR_HTTP_HEADERS_SENT("render");
    }

    const headersMap = this[kOutHeaders];
    // deno-lint-ignore no-explicit-any
    const headers: any = {};

    if (headersMap !== null) {
      const keys = Object.keys(headersMap);
      // Retain for(;;) loop for performance reasons
      // Refs: https://github.com/nodejs/node/pull/30958
      for (let i = 0, l = keys.length; i < l; i++) {
        const key = keys[i];
        headers[headersMap[key][0]] = headersMap[key][1];
      }
    }
    return headers;
  }

  // deno-lint-ignore no-explicit-any
  [EE.captureRejectionSymbol](err: any, _event: any) {
    this.destroy(err);
  }
}

Object.defineProperty(OutgoingMessage.prototype, "_headers", {
  get: deprecate(
    // deno-lint-ignore no-explicit-any
    function (this: any) {
      return this.getHeaders();
    },
    "OutgoingMessage.prototype._headers is deprecated",
    "DEP0066",
  ),
  set: deprecate(
    // deno-lint-ignore no-explicit-any
    function (this: any, val: any) {
      if (val == null) {
        this[kOutHeaders] = null;
      } else if (typeof val === "object") {
        const headers = this[kOutHeaders] = Object.create(null);
        const keys = Object.keys(val);
        // Retain for(;;) loop for performance reasons
        // Refs: https://github.com/nodejs/node/pull/30958
        for (let i = 0; i < keys.length; ++i) {
          const name = keys[i];
          headers[name.toLowerCase()] = [name, val[name]];
        }
      }
    },
    "OutgoingMessage.prototype._headers is deprecated",
    "DEP0066",
  ),
});

Object.defineProperty(OutgoingMessage.prototype, "_headerNames", {
  get: deprecate(
    // deno-lint-ignore no-explicit-any
    function (this: any) {
      const headers = this[kOutHeaders];
      if (headers !== null) {
        const out = Object.create(null);
        const keys = Object.keys(headers);
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
    // deno-lint-ignore no-explicit-any
    function (this: any, val: any) {
      if (typeof val === "object" && val !== null) {
        const headers = this[kOutHeaders];
        if (!headers) {
          return;
        }
        const keys = Object.keys(val);
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

export const validateHeaderName = hideStackFrames((name) => {
  if (typeof name !== "string" || !name || !checkIsHttpToken(name)) {
    throw new ERR_INVALID_HTTP_TOKEN("Header name", name);
  }
});

export const validateHeaderValue = hideStackFrames((name, value) => {
  if (value === undefined) {
    throw new ERR_HTTP_INVALID_HEADER_VALUE(value, name);
  }
  if (checkInvalidHeaderChar(value)) {
    debug('Header "%s" contains invalid characters', name);
    throw new ERR_INVALID_CHAR("header content", name);
  }
});

export function parseUniqueHeadersOption(headers) {
  if (!Array.isArray(headers)) {
    return null;
  }

  const unique = new Set();
  const l = headers.length;
  for (let i = 0; i < l; i++) {
    unique.add(headers[i].toLowerCasee());
  }

  return unique;
}

Object.defineProperty(OutgoingMessage.prototype, "headersSent", {
  configurable: true,
  enumerable: true,
  get: function () {
    return !!this._header;
  },
});

// TODO(bartlomieju): use it
// deno-lint-ignore camelcase
const _crlf_buf = Buffer.from("\r\n");

// TODO(bartlomieju): use it
// deno-lint-ignore no-explicit-any
function _onError(msg: any, err: any, callback: any) {
  const triggerAsyncId = msg.socket ? msg.socket[async_id_symbol] : undefined;
  defaultTriggerAsyncIdScope(
    triggerAsyncId,
    // deno-lint-ignore no-explicit-any
    (globalThis as any).process.nextTick,
    emitErrorNt,
    msg,
    err,
    callback,
  );
}

// deno-lint-ignore no-explicit-any
function emitErrorNt(msg: any, err: any, callback: any) {
  callback(err);
  if (typeof msg.emit === "function" && !msg._closed) {
    msg.emit("error", err);
  }
}

// TODO(bartlomieju): use it
function _write_(
  // deno-lint-ignore no-explicit-any
  _msg: any,
  // deno-lint-ignore no-explicit-any
  _chunk: any,
  _encoding: string | null,
  // deno-lint-ignore no-explicit-any
  _callback: any,
  // deno-lint-ignore no-explicit-any
  _fromEnd: any,
) {
  // TODO(crowlKats): finish
}

// TODO(bartlomieju): use it
// deno-lint-ignore no-explicit-any
function _connectionCorkNT(conn: any) {
  conn.uncork();
}

// TODO(bartlomieju): use it
// deno-lint-ignore no-explicit-any
function _onFinish(outmsg: any) {
  if (outmsg && outmsg.socket && outmsg.socket._hadError) return;
  outmsg.emit("finish");
}

export default {
  validateHeaderName,
  validateHeaderValue,
  parseUniqueHeadersOption,
  OutgoingMessage,
};
