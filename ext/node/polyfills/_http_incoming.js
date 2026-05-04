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

// Ported from Node.js lib/_http_incoming.js

// deno-lint-ignore-file prefer-primordials

import { primordials } from "ext:core/mod.js";
const {
  ObjectDefineProperty,
  ObjectSetPrototypeOf,
  Symbol,
} = primordials;

import { finished, Readable } from "node:stream";
import { nextTick } from "ext:deno_node/_next_tick.ts";

const kHeaders = Symbol("kHeaders");
const kHeadersDistinct = Symbol("kHeadersDistinct");
const kHeadersCount = Symbol("kHeadersCount");
const kTrailers = Symbol("kTrailers");
const kTrailersDistinct = Symbol("kTrailersDistinct");
const kTrailersCount = Symbol("kTrailersCount");

function readStart(socket) {
  if (socket && !socket._paused && socket.readable) {
    socket.resume();
  }
}

function readStop(socket) {
  if (socket) {
    socket.pause();
  }
}

/* Abstract base class for ServerRequest and ClientResponse. */
function IncomingMessage(socket, options) {
  let streamOptions;

  if (socket) {
    streamOptions = {
      highWaterMark: options?.highWaterMark ?? socket.readableHighWaterMark,
    };
  }

  Readable.call(this, streamOptions);

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
  this._dumped = false;
}
ObjectSetPrototypeOf(IncomingMessage.prototype, Readable.prototype);
ObjectSetPrototypeOf(IncomingMessage, Readable);

ObjectDefineProperty(IncomingMessage.prototype, "connection", {
  __proto__: null,
  get: function () {
    return this.socket;
  },
  set: function (val) {
    this.socket = val;
  },
});

ObjectDefineProperty(IncomingMessage.prototype, "headers", {
  __proto__: null,
  get: function () {
    if (!this[kHeaders]) {
      this[kHeaders] = {};

      const src = this.rawHeaders;
      const dst = this[kHeaders];

      for (let n = 0; n < this[kHeadersCount]; n += 2) {
        this._addHeaderLine(src[n + 0], src[n + 1], dst);
      }
    }
    return this[kHeaders];
  },
  set: function (val) {
    this[kHeaders] = val;
  },
});

ObjectDefineProperty(IncomingMessage.prototype, "headersDistinct", {
  __proto__: null,
  get: function () {
    if (!this[kHeadersDistinct]) {
      this[kHeadersDistinct] = {};

      const src = this.rawHeaders;
      const dst = this[kHeadersDistinct];

      for (let n = 0; n < this[kHeadersCount]; n += 2) {
        this._addHeaderLineDistinct(src[n + 0], src[n + 1], dst);
      }
    }
    return this[kHeadersDistinct];
  },
  set: function (val) {
    this[kHeadersDistinct] = val;
  },
});

ObjectDefineProperty(IncomingMessage.prototype, "trailers", {
  __proto__: null,
  get: function () {
    if (!this[kTrailers]) {
      this[kTrailers] = {};

      const src = this.rawTrailers;
      const dst = this[kTrailers];

      for (let n = 0; n < this[kTrailersCount]; n += 2) {
        this._addHeaderLine(src[n + 0], src[n + 1], dst);
      }
    }
    return this[kTrailers];
  },
  set: function (val) {
    this[kTrailers] = val;
  },
});

ObjectDefineProperty(IncomingMessage.prototype, "trailersDistinct", {
  __proto__: null,
  get: function () {
    if (!this[kTrailersDistinct]) {
      this[kTrailersDistinct] = {};

      const src = this.rawTrailers;
      const dst = this[kTrailersDistinct];

      for (let n = 0; n < this[kTrailersCount]; n += 2) {
        this._addHeaderLineDistinct(src[n + 0], src[n + 1], dst);
      }
    }
    return this[kTrailersDistinct];
  },
  set: function (val) {
    this[kTrailersDistinct] = val;
  },
});

IncomingMessage.prototype.setTimeout = function setTimeout(msecs, callback) {
  if (callback) {
    this.on("timeout", callback);
  }
  this.socket.setTimeout(msecs);
  return this;
};

// The parser pushes body data directly via push(). We just need to
// unpause the underlying socket so data flows.
IncomingMessage.prototype._read = function _read(_n) {
  if (!this._consuming) {
    this._readableState.readingMore = false;
    this._consuming = true;
  }

  if (this.socket.readable) {
    readStart(this.socket);
  }
};

IncomingMessage.prototype._destroy = function _destroy(err, cb) {
  if (!this.readableEnded || !this.complete) {
    this.aborted = true;
    this.emit("aborted");
  }

  if (this.socket && !this.socket.destroyed && this.aborted) {
    this.socket.destroy(err);
    const cleanup = finished(this.socket, (e) => {
      if (e?.code === "ERR_STREAM_PREMATURE_CLOSE") {
        e = null;
      }
      cleanup();
      nextTick(onError, this, e || err, cb);
    });
  } else {
    nextTick(onError, this, err, cb);
  }
};

IncomingMessage.prototype._addHeaderLines = _addHeaderLines;
function _addHeaderLines(headers, n) {
  if (headers?.length) {
    let dest;
    if (this.complete) {
      this.rawTrailers = headers;
      this[kTrailersCount] = n;
      dest = this[kTrailers];
    } else {
      this.rawHeaders = headers;
      this[kHeadersCount] = n;
      dest = this[kHeaders];
    }

    if (dest) {
      for (let i = 0; i < n; i += 2) {
        this._addHeaderLine(headers[i], headers[i + 1], dest);
      }
    }
  }
}

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
      if (field === "Location" || field === "location") return "location";
      if (field === "If-Match" || field === "if-match") {
        return "\u0000if-match";
      }
      break;
    case 10:
      if (field === "User-Agent" || field === "user-agent") {
        return "user-agent";
      }
      if (field === "Set-Cookie" || field === "set-cookie") return "\u0001";
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
      if (
        field === "Proxy-Authorization" || field === "proxy-authorization"
      ) {
        return "proxy-authorization";
      }
      if (
        field === "If-Unmodified-Since" || field === "if-unmodified-since"
      ) {
        return "if-unmodified-since";
      }
      break;
  }
  if (lowercased) {
    return "\u0000" + field;
  }
  return matchKnownFields(field.toLowerCase(), true);
}

IncomingMessage.prototype._addHeaderLine = _addHeaderLine;
function _addHeaderLine(field, value, dest) {
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

IncomingMessage.prototype._addHeaderLineDistinct = _addHeaderLineDistinct;
function _addHeaderLineDistinct(field, value, dest) {
  field = field.toLowerCase();
  if (!dest[field]) {
    dest[field] = [value];
  } else {
    dest[field].push(value);
  }
}

IncomingMessage.prototype._dumpAndCloseReadable =
  function _dumpAndCloseReadable() {
    this._dumped = true;
    this._readableState.ended = true;
    this._readableState.endEmitted = true;
    this._readableState.destroyed = true;
    this._readableState.closed = true;
    this._readableState.closeEmitted = true;
  };

IncomingMessage.prototype._dump = function _dump() {
  if (!this._dumped) {
    this._dumped = true;
    this.removeAllListeners("data");
    this.resume();
  }
};

function onError(self, error, cb) {
  if (self.listenerCount("error") === 0) {
    cb();
  } else {
    cb(error);
  }
}

export { IncomingMessage, readStart, readStop };

export default {
  IncomingMessage,
  readStart,
  readStop,
};
