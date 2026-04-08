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

// Ported from Node.js lib/_http_common.js

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any no-this-alias

import { primordials } from "ext:core/mod.js";
const {
  MathMin,
  Symbol,
} = primordials;

import {
  allMethods,
  HTTPParser,
  methods,
} from "ext:deno_node/internal_binding/http_parser.ts";
import { IncomingMessage, readStart, readStop } from "node:_http_incoming";

const kIncomingMessage = Symbol("IncomingMessage");
const kSkipPendingData = Symbol("SkipPendingData");
const kOnMessageBegin = HTTPParser.kOnMessageBegin | 0;
const kOnHeaders = HTTPParser.kOnHeaders | 0;
const kOnHeadersComplete = HTTPParser.kOnHeadersComplete | 0;
const kOnBody = HTTPParser.kOnBody | 0;
const kOnMessageComplete = HTTPParser.kOnMessageComplete | 0;
const kOnExecute = HTTPParser.kOnExecute | 0;
const kOnTimeout = HTTPParser.kOnTimeout | 0;

const MAX_HEADER_PAIRS = 2000;

// Simple FreeList implementation (matches Node.js internal/freelist)
class FreeList {
  name: string;
  max: number;
  ctor: () => any;
  list: any[];

  constructor(name: string, max: number, ctor: () => any) {
    this.name = name;
    this.max = max;
    this.ctor = ctor;
    this.list = [];
  }

  alloc() {
    return this.list.length > 0 ? this.list.pop() : this.ctor();
  }

  free(obj: any) {
    if (this.list.length < this.max) {
      this.list.push(obj);
      return true;
    }
    return false;
  }
}

// Only called in the slow case where slow means
// that the request headers were either fragmented
// across multiple TCP packets or too large to be
// processed in a single run. This method is also
// called to process trailing HTTP headers.
function parserOnHeaders(this: any, headers: string[], url: string) {
  // Once we exceeded headers limit - stop collecting them
  const capacity = this.maxHeaderPairs - this._headers.length;
  if (this.maxHeaderPairs <= 0 || capacity >= headers.length) {
    this._headers.push(...headers);
  } else if (capacity > 0) {
    this._headers.push(...headers.slice(0, capacity));
  }
  this._url += url;
}

const HTTP_VERSION_1_1 = "1.1";

// `headers` and `url` are set only if .onHeaders() has not been called for
// this request.
// `url` is not set for response parsers but that's not applicable here since
// all our parsers are request parsers.
function parserOnHeadersComplete(
  this: any,
  versionMajor: number,
  versionMinor: number,
  headers: string[] | undefined,
  method: number | undefined,
  url: string | undefined,
  statusCode: number | undefined,
  statusMessage: string | undefined,
  upgrade: boolean,
  shouldKeepAlive: boolean,
) {
  const parser = this;
  const { socket } = parser;

  if (headers === undefined) {
    headers = parser._headers;
    parser._headers = [];
  }

  if (url === undefined) {
    url = parser._url;
    parser._url = "";
  }

  // Parser is also used by http client
  const ParserIncomingMessage = (socket?.server?.[kIncomingMessage]) ||
    IncomingMessage;

  const incoming = parser.incoming = new ParserIncomingMessage(socket);
  incoming.httpVersionMajor = versionMajor;
  incoming.httpVersionMinor = versionMinor;
  incoming.httpVersion = versionMajor === 1 && versionMinor === 1
    ? HTTP_VERSION_1_1
    : `${versionMajor}.${versionMinor}`;
  incoming.joinDuplicateHeaders = socket?.server?.joinDuplicateHeaders ||
    parser.joinDuplicateHeaders;
  incoming.url = url;
  incoming.upgrade = upgrade;

  let n = headers!.length;

  // If parser.maxHeaderPairs <= 0 assume that there's no limit.
  if (parser.maxHeaderPairs > 0) {
    n = MathMin(n, parser.maxHeaderPairs);
  }

  incoming._addHeaderLines(headers, n);

  if (typeof method === "number") {
    // server only
    incoming.method = allMethods[method];
  } else {
    // client only
    incoming.statusCode = statusCode;
    incoming.statusMessage = statusMessage;
  }

  return parser.onIncoming(incoming, shouldKeepAlive);
}

// deno-lint-ignore no-node-globals
function parserOnBody(this: any, b: Buffer) {
  const stream = this.incoming;

  // If the stream has already been removed, then drop it.
  if (stream === null || stream[kSkipPendingData]) {
    return;
  }

  // Pretend this was the result of a stream._read call.
  if (!stream._dumped) {
    const ret = stream.push(b);
    if (!ret) {
      readStop(this.socket);
    }
  }
}

function parserOnMessageComplete(this: any) {
  const parser = this;
  const stream = parser.incoming;

  if (stream !== null && !stream[kSkipPendingData]) {
    stream.complete = true;
    // Emit any trailing headers.
    const headers = parser._headers;
    if (headers.length) {
      stream._addHeaderLines(headers, headers.length);
      parser._headers = [];
      parser._url = "";
    }

    // For emit end event
    stream.push(null);
  }

  // Force to read the next incoming message
  readStart(parser.socket);
}

const parsers = new FreeList("parsers", 1000, function parsersCb() {
  const parser = new (HTTPParser as any)();

  cleanParser(parser);

  parser[kOnHeaders] = parserOnHeaders;
  parser[kOnHeadersComplete] = parserOnHeadersComplete;
  parser[kOnBody] = parserOnBody;
  parser[kOnMessageComplete] = parserOnMessageComplete;

  return parser;
});

function closeParserInstance(parser: any) {
  parser.close();
}

// Free the parser and also break any links that it
// might have to any other things.
function freeParser(parser: any, req: any, socket: any) {
  if (parser) {
    if (parser._consumed) {
      parser.unconsume();
    }
    parser.remove();
    cleanParser(parser);
    if (parsers.free(parser) === false) {
      // Make sure the parser's stack has unwound before deleting the
      // corresponding C++ object through .close().
      setTimeout(closeParserInstance, 0, parser);
    } else {
      parser.free();
    }
  }
  if (req) {
    req.parser = null;
  }
  if (socket) {
    socket.parser = null;
  }
}

const tokenRegExp = /^[\^_`a-zA-Z\-0-9!#$%&'*+.|~]+$/;

function checkIsHttpToken(val: string) {
  return tokenRegExp.test(val);
}

const headerCharRegex = /[^\t\x20-\x7e\x80-\xff]/;

function checkInvalidHeaderChar(val: string) {
  return headerCharRegex.test(val);
}

function cleanParser(parser: any) {
  parser._headers = [];
  parser._url = "";
  parser.socket = null;
  parser.incoming = null;
  parser.outgoing = null;
  parser.maxHeaderPairs = MAX_HEADER_PAIRS;
  parser[kOnMessageBegin] = null;
  parser[kOnExecute] = null;
  parser[kOnTimeout] = null;
  parser._consumed = false;
  parser.onIncoming = null;
  parser.joinDuplicateHeaders = null;
}

// deno-lint-ignore no-node-globals
function prepareError(err: any, parser: any, rawPacket?: Buffer) {
  err.rawPacket = rawPacket || parser.getCurrentBuffer();
  if (typeof err.reason === "string") {
    err.message = `Parse Error: ${err.reason}`;
  }
}

function isLenient() {
  // Deno doesn't have --insecure-http-parser flag (yet)
  return false;
}

export const CRLF = "\r\n";
export const chunkExpression = /(?:^|\W)chunked(?:$|\W)/i;
export const continueExpression = /(?:^|\W)100-continue(?:$|\W)/i;

export {
  checkInvalidHeaderChar as _checkInvalidHeaderChar,
  checkIsHttpToken as _checkIsHttpToken,
  freeParser,
  HTTPParser,
  IncomingMessage,
  isLenient,
  kIncomingMessage,
  kOnBody,
  kOnHeaders,
  kOnHeadersComplete,
  kOnMessageComplete,
  kSkipPendingData,
  methods,
  parsers,
  prepareError,
  readStart,
  readStop,
};

export default {
  _checkInvalidHeaderChar: checkInvalidHeaderChar,
  _checkIsHttpToken: checkIsHttpToken,
  chunkExpression,
  CRLF,
  continueExpression,
  freeParser,
  HTTPParser,
  IncomingMessage,
  isLenient,
  kIncomingMessage,
  kSkipPendingData,
  methods,
  parsers,
  prepareError,
  readStart,
  readStop,
};
