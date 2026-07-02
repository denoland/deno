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

import { core, primordials } from "ext:core/mod.js";
const {
  ArrayPrototypePop,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  MathMin,
  SafeArrayIterator,
  RegExpPrototypeTest,
  SafeRegExp,
  String,
  StringFromCharCode,
  StringPrototypeCharCodeAt,
  Symbol,
  TypedArrayPrototypeGetBuffer,
  TypedArrayPrototypeGetByteLength,
  TypedArrayPrototypeGetByteOffset,
  Uint8Array,
} = primordials;
import { setImmediate } from "node:timers";
const {
  allMethods,
  HTTPParser,
  methods,
} = core.loadExtScript("ext:deno_node/internal_binding/http_parser.ts");
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
  constructor(name, max, ctor) {
    this.name = name;
    this.max = max;
    this.ctor = ctor;
    this.list = [];
  }

  alloc() {
    return this.list.length > 0 ? ArrayPrototypePop(this.list) : this.ctor();
  }

  free(obj) {
    if (this.list.length < this.max) {
      ArrayPrototypePush(this.list, obj);
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
function parserOnHeaders(headers, url) {
  // Once we exceeded headers limit - stop collecting them
  const capacity = this.maxHeaderPairs - this._headers.length;
  if (this.maxHeaderPairs <= 0 || capacity >= headers.length) {
    ArrayPrototypePush(this._headers, ...new SafeArrayIterator(headers));
  } else if (capacity > 0) {
    ArrayPrototypePush(
      this._headers,
      ...new SafeArrayIterator(ArrayPrototypeSlice(headers, 0, capacity)),
    );
  }
  this._url += url;
}

const HTTP_VERSION_1_1 = "1.1";

// `headers` and `url` are set only if .onHeaders() has not been called for
// this request.
// `url` is not set for response parsers but that's not applicable here since
// all our parsers are request parsers.
function parserOnHeadersComplete(
  versionMajor,
  versionMinor,
  headers,
  method,
  url,
  statusCode,
  statusMessage,
  upgrade,
  shouldKeepAlive,
) {
  // deno-lint-ignore no-this-alias
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

  const incoming = parser.incoming = new ParserIncomingMessage(socket, {
    highWaterMark: socket?.server?.highWaterMark,
  });
  incoming.httpVersionMajor = versionMajor;
  incoming.httpVersionMinor = versionMinor;
  incoming.httpVersion = versionMajor === 1 && versionMinor === 1
    ? HTTP_VERSION_1_1
    : `${versionMajor}.${versionMinor}`;
  incoming.joinDuplicateHeaders = socket?.server?.joinDuplicateHeaders ||
    parser.joinDuplicateHeaders;
  incoming.url = url;
  incoming.upgrade = upgrade;

  let n = headers.length;

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

function parserOnBody(b) {
  const stream = this.incoming;

  // If the stream has already been removed, then drop it.
  if (stream === null || stream[kSkipPendingData]) {
    return;
  }

  // Pretend this was the result of a stream._read call.
  if (!stream._dumped) {
    // Mark as consuming so resOnFinish does not auto-dump a
    // paused request that already had data delivered.
    stream._consuming = true;
    // deno-lint-ignore prefer-primordials
    const ret = stream.push(b);
    if (!ret) {
      readStop(this.socket);
    }
  }
}

function parserOnMessageComplete() {
  // deno-lint-ignore no-this-alias
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
    // deno-lint-ignore prefer-primordials
    stream.push(null);
  }

  // Force to read the next incoming message
  readStart(parser.socket);
}

const parsers = new FreeList("parsers", 1000, function parsersCb() {
  const parser = new HTTPParser();

  cleanParser(parser);

  parser[kOnHeaders] = parserOnHeaders;
  parser[kOnHeadersComplete] = parserOnHeadersComplete;
  parser[kOnBody] = parserOnBody;
  parser[kOnMessageComplete] = parserOnMessageComplete;

  return parser;
});

function closeParserInstance(parser) {
  parser.close();
}

// Free the parser and also break any links that it
// might have to any other things.
function freeParser(parser, req, socket) {
  if (parser) {
    if (parser._consumed) {
      parser.unconsume();
    }
    parser.remove();
    cleanParser(parser);
    if (parsers.free(parser) === false) {
      setImmediate(closeParserInstance, parser);
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

// deno-fmt-ignore
const validTokenChars = new Uint8Array([
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0,
  1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0,
  0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
  1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1,
  1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
  1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
]);

function checkIsHttpToken(val) {
  // Table-driven scan for all lengths. The previous code fell back to a regex
  // for val.length >= 10, which caught common header names (content-type,
  // content-length, x-powered-by, ...) and showed up as RegExpExecSlow +
  // string flattening in the hot path. validTokenChars[code] is undefined for
  // code > 255 (non-ASCII), which is correctly falsy.
  const len = val.length;
  if (len === 0) return false;
  for (let i = 0; i < len; i++) {
    if (validTokenChars[StringPrototypeCharCodeAt(val, i)] !== 1) {
      return false;
    }
  }
  return true;
}

// Invalid header value chars (RFC 7230 / Node): control chars except HT, DEL,
// and any UTF-16 code unit > 0xff. 1 = invalid. Table-driven instead of a regex
// (the regex forced a per-call string flatten -- ~3-4% of the Express hot path).
const invalidHeaderCharTable = new Uint8Array(256);
for (let i = 0; i <= 0x1f; i++) invalidHeaderCharTable[i] = 1;
invalidHeaderCharTable[0x09] = 0; // HT is allowed
invalidHeaderCharTable[0x7f] = 1; // DEL
// Lenient (insecure parser): only NUL, LF, CR (and code units > 0xff) invalid.
const lenientInvalidHeaderCharTable = new Uint8Array(256);
lenientInvalidHeaderCharTable[0x00] = 1;
lenientInvalidHeaderCharTable[0x0a] = 1;
lenientInvalidHeaderCharTable[0x0d] = 1;

function checkInvalidHeaderChar(val, lenient = false) {
  const table = lenient
    ? lenientInvalidHeaderCharTable
    : invalidHeaderCharTable;
  // Node validates via a regex, which coerces its argument to a string; a
  // non-string value (e.g. setHeader(name, null) or a number) is stringified
  // rather than throwing on a missing `.length`. Match that instead of the
  // char-table scan choking on `null.length`.
  const str = typeof val === "string" ? val : String(val);
  for (let i = 0; i < str.length; i++) {
    const code = StringPrototypeCharCodeAt(str, i);
    if (code > 0xff || table[code] === 1) {
      return true;
    }
  }
  return false;
}

function cleanParser(parser) {
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
  parser._asyncResource = null;
  parser._lastRawPacket = null;
}

// The Rust binding collapses every llhttp errno into the generic HPE_ERROR;
// Node.js surfaces the specific code (e.g. HPE_INVALID_TRANSFER_ENCODING)
// straight from llhttp. Recover the smuggling-relevant case in JS by checking
// whether the raw packet combined Content-Length with Transfer-Encoding:
// chunked, a combination RFC 9112 forbids precisely because it enables request
// smuggling.
const contentLengthHeaderRegex = new SafeRegExp(
  "^content-length[ \\t]*:",
  "im",
);
const transferEncodingChunkedRegex = new SafeRegExp(
  "^transfer-encoding[ \\t]*:[^\\r\\n]*\\bchunked\\b",
  "im",
);

function prepareError(err, parser, rawPacket) {
  err.rawPacket = rawPacket || parser.getCurrentBuffer();
  if (
    err.code === "HPE_ERROR" && err.rawPacket &&
    TypedArrayPrototypeGetByteLength(err.rawPacket) > 0
  ) {
    const headers = headerSegmentLatin1(err.rawPacket);
    if (
      RegExpPrototypeTest(contentLengthHeaderRegex, headers) &&
      RegExpPrototypeTest(transferEncodingChunkedRegex, headers)
    ) {
      err.code = "HPE_INVALID_TRANSFER_ENCODING";
      err.reason = "Transfer-Encoding can't be present with Content-Length";
    }
  }
  if (typeof err.reason === "string") {
    err.message = `Parse Error: ${err.reason}`;
  }
}

// Decode the header section (everything before the first CRLF CRLF) of a
// raw HTTP packet as a latin1 string. Returns the full packet decoded if no
// header terminator is present. Operates on Uint8Array or Buffer.
function headerSegmentLatin1(rawPacket) {
  const view = new Uint8Array(
    TypedArrayPrototypeGetBuffer(rawPacket),
    TypedArrayPrototypeGetByteOffset(rawPacket),
    TypedArrayPrototypeGetByteLength(rawPacket),
  );
  const len = view.length;
  let end = len;
  for (let i = 0; i + 3 < len; i++) {
    if (
      view[i] === 0x0d && view[i + 1] === 0x0a &&
      view[i + 2] === 0x0d && view[i + 3] === 0x0a
    ) {
      end = i;
      break;
    }
  }
  let out = "";
  for (let i = 0; i < end; i++) {
    out += StringFromCharCode(view[i]);
  }
  return out;
}

function isLenient() {
  // Deno doesn't have --insecure-http-parser flag (yet)
  return false;
}

export const CRLF = "\r\n";
export const chunkExpression = new SafeRegExp("(?:^|\\W)chunked(?:$|\\W)", "i");
export const continueExpression = new SafeRegExp(
  "(?:^|\\W)100-continue(?:$|\\W)",
  "i",
);

export {
  checkInvalidHeaderChar as _checkInvalidHeaderChar,
  checkIsHttpToken as _checkIsHttpToken,
  freeParser,
  HTTPParser,
  IncomingMessage,
  isLenient,
  kIncomingMessage,
  kOnBody,
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
