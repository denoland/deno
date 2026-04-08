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

// deno-lint-ignore-file no-explicit-any prefer-primordials

import { HTTPParser as NativeHTTPParser } from "ext:core/ops";

// Method names indexed by llhttp method enum values.
// Order must match llhttp_method_t in llhttp.h.
export const methods = [
  "DELETE",
  "GET",
  "HEAD",
  "POST",
  "PUT",
  "CONNECT",
  "OPTIONS",
  "TRACE",
  "COPY",
  "LOCK",
  "MKCOL",
  "MOVE",
  "PROPFIND",
  "PROPPATCH",
  "SEARCH",
  "UNLOCK",
  "BIND",
  "REBIND",
  "UNBIND",
  "ACL",
  "REPORT",
  "MKACTIVITY",
  "CHECKOUT",
  "MERGE",
  "M-SEARCH",
  "NOTIFY",
  "SUBSCRIBE",
  "UNSUBSCRIBE",
  "PATCH",
  "PURGE",
  "MKCALENDAR",
  "LINK",
  "UNLINK",
  "SOURCE",
  "QUERY",
];

export const allMethods = [
  "DELETE",
  "GET",
  "HEAD",
  "POST",
  "PUT",
  "CONNECT",
  "OPTIONS",
  "TRACE",
  "COPY",
  "LOCK",
  "MKCOL",
  "MOVE",
  "PROPFIND",
  "PROPPATCH",
  "SEARCH",
  "UNLOCK",
  "BIND",
  "REBIND",
  "UNBIND",
  "ACL",
  "REPORT",
  "MKACTIVITY",
  "CHECKOUT",
  "MERGE",
  "M-SEARCH",
  "NOTIFY",
  "SUBSCRIBE",
  "UNSUBSCRIBE",
  "PATCH",
  "PURGE",
  "MKCALENDAR",
  "LINK",
  "UNLINK",
  "SOURCE",
  "PRI",
  "DESCRIBE",
  "ANNOUNCE",
  "SETUP",
  "PLAY",
  "PAUSE",
  "TEARDOWN",
  "GET_PARAMETER",
  "SET_PARAMETER",
  "REDIRECT",
  "RECORD",
  "FLUSH",
  "QUERY",
];

// Callback indices - used as indexed properties on the parser instance.
const kOnMessageBegin = 0;
const kOnHeaders = 1;
const kOnHeadersComplete = 2;
const kOnBody = 3;
const kOnMessageComplete = 4;
const kOnExecute = 5;
const kOnTimeout = 6;

/**
 * JS wrapper around the native llhttp-based HTTPParser cppgc object.
 *
 * Node.js's `_http_common.js` sets callbacks as indexed properties:
 *   parser[kOnHeaders] = function(headers, url) { ... }
 *   parser[kOnHeadersComplete] = function(major, minor, headers, ...) { ... }
 *
 * The native parser reads these during execute() and calls them
 * synchronously from the C callbacks.
 */
export function HTTPParser(this: any, type?: number) {
  // Create the native cppgc parser
  this._native = new NativeHTTPParser();

  // If type is provided in constructor, initialize immediately
  if (type !== undefined) {
    this._native.initialize(type, 0, 0);
  }
}

HTTPParser.prototype.initialize = function (
  this: any,
  type: number,
  _options?: any,
  maxHeaderSize?: number,
  lenientFlags?: number,
) {
  this._native.initialize(
    type,
    maxHeaderSize ?? 0,
    lenientFlags ?? 0,
  );
};

HTTPParser.prototype.execute = function (
  this: any,
  buffer: Uint8Array,
  offset?: number,
  length?: number,
) {
  // Node.js calls execute(buffer) or execute(buffer, offset, length)
  let data: Uint8Array;
  if (
    offset !== undefined && length !== undefined &&
    (offset !== 0 || length !== buffer.length)
  ) {
    data = buffer.subarray(offset, offset + length);
  } else {
    data = buffer;
  }

  // Pass `this` (the JS wrapper) as the callbacks object. The native
  // execute() reads indexed properties (kOnHeaders etc.) from it.
  return this._native.execute(
    this,
    new Uint8Array(data.buffer, data.byteOffset, data.byteLength),
  );
};

HTTPParser.prototype.finish = function (this: any) {
  return this._native.finish();
};

HTTPParser.prototype.pause = function (this: any) {
  this._native.pause();
};

HTTPParser.prototype.resume = function (this: any) {
  this._native.resume();
};

HTTPParser.prototype.close = function (this: any) {
  this._native.close();
};

HTTPParser.prototype.free = function (this: any) {
  this._native.free();
};

HTTPParser.prototype.remove = function (this: any) {
  this._native.remove();
};

HTTPParser.prototype.getCurrentBuffer = function (this: any) {
  return this._native.getCurrentBuffer();
};

// consume/unconsume - server optimization (not yet implemented)
HTTPParser.prototype.consume = function (_handle: any) {
  // TODO(@bartlomieju): implement StreamListener-based consume for server optimization
};

HTTPParser.prototype.unconsume = function () {
  // TODO(@bartlomieju): implement unconsume
};

// Static constants
HTTPParser.REQUEST = 1;
HTTPParser.RESPONSE = 2;
HTTPParser.kOnMessageBegin = kOnMessageBegin;
HTTPParser.kOnHeaders = kOnHeaders;
HTTPParser.kOnHeadersComplete = kOnHeadersComplete;
HTTPParser.kOnBody = kOnBody;
HTTPParser.kOnMessageComplete = kOnMessageComplete;
HTTPParser.kOnExecute = kOnExecute;
HTTPParser.kOnTimeout = kOnTimeout;
HTTPParser.kLenientNone = 0;
HTTPParser.kLenientHeaders = 1;
HTTPParser.kLenientChunkedLength = 2;
HTTPParser.kLenientKeepAlive = 4;
HTTPParser.kLenientTransferEncoding = 8;
HTTPParser.kLenientVersion = 16;
HTTPParser.kLenientDataAfterClose = 32;
HTTPParser.kLenientOptionalLFAfterCR = 64;
HTTPParser.kLenientOptionalCRLFAfterChunk = 128;
HTTPParser.kLenientOptionalCRBeforeLF = 256;
HTTPParser.kLenientSpacesAfterChunkSize = 512;
HTTPParser.kLenientAll = 1023;
