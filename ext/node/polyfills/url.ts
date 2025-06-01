// Copyright 2018-2025 the Deno authors. MIT license.
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

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  ERR_INVALID_FILE_URL_HOST,
  ERR_INVALID_FILE_URL_PATH,
  ERR_INVALID_URL,
  ERR_INVALID_URL_SCHEME,
} from "ext:deno_node/internal/errors.ts";
import { validateString } from "ext:deno_node/internal/validators.mjs";
import {
  CHAR_0,
  CHAR_9,
  CHAR_AT,
  CHAR_BACKWARD_SLASH,
  CHAR_CARRIAGE_RETURN,
  CHAR_CIRCUMFLEX_ACCENT,
  CHAR_DOT,
  CHAR_DOUBLE_QUOTE,
  CHAR_FORM_FEED,
  CHAR_FORWARD_SLASH,
  CHAR_GRAVE_ACCENT,
  CHAR_HASH,
  CHAR_HYPHEN_MINUS,
  CHAR_LEFT_ANGLE_BRACKET,
  CHAR_LEFT_CURLY_BRACKET,
  CHAR_LEFT_SQUARE_BRACKET,
  CHAR_LINE_FEED,
  CHAR_LOWERCASE_A,
  CHAR_LOWERCASE_Z,
  CHAR_NO_BREAK_SPACE,
  CHAR_PERCENT,
  CHAR_PLUS,
  CHAR_QUESTION_MARK,
  CHAR_RIGHT_ANGLE_BRACKET,
  CHAR_RIGHT_CURLY_BRACKET,
  CHAR_RIGHT_SQUARE_BRACKET,
  CHAR_SEMICOLON,
  CHAR_SINGLE_QUOTE,
  CHAR_SPACE,
  CHAR_TAB,
  CHAR_UNDERSCORE,
  CHAR_UPPERCASE_A,
  CHAR_UPPERCASE_Z,
  CHAR_VERTICAL_LINE,
  CHAR_ZERO_WIDTH_NOBREAK_SPACE,
} from "ext:deno_node/path/_constants.ts";
import * as path from "node:path";
import {
  domainToASCII as idnaToASCII,
  domainToUnicode as idnaToUnicode,
} from "ext:deno_node/internal/idna.ts";
import { isWindows, osType } from "ext:deno_node/_util/os.ts";
import { encodeStr, hexTable } from "ext:deno_node/internal/querystring.ts";
import querystring from "node:querystring";
import type { ParsedUrlQuery, ParsedUrlQueryInput } from "node:querystring";
import { URL, URLSearchParams } from "ext:deno_url/00_url.js";

const forwardSlashRegEx = /\//g;
const percentRegEx = /%/g;
const backslashRegEx = /\\/g;
const newlineRegEx = /\n/g;
const carriageReturnRegEx = /\r/g;
const tabRegEx = /\t/g;
// Reference: RFC 3986, RFC 1808, RFC 2396

// define these here so at least they only have to be
// compiled once on the first module load.
const protocolPattern = /^[a-z0-9.+-]+:/i;
const portPattern = /:[0-9]*$/;
const hostPattern = /^\/\/[^@/]+@[^@/]+/;
// Special case for a simple path URL
const simplePathPattern = /^(\/\/?(?!\/)[^?\s]*)(\?[^\s]*)?$/;
// Protocols that can allow "unsafe" and "unwise" chars.
const unsafeProtocol = new Set(["javascript", "javascript:"]);
// Protocols that never have a hostname.
const hostlessProtocol = new Set(["javascript", "javascript:"]);
// Protocols that always contain a // bit.
const slashedProtocol = new Set([
  "http",
  "http:",
  "https",
  "https:",
  "ftp",
  "ftp:",
  "gopher",
  "gopher:",
  "file",
  "file:",
  "ws",
  "ws:",
  "wss",
  "wss:",
]);

const hostnameMaxLen = 255;

// These characters do not need escaping:
// ! - . _ ~
// ' ( ) * :
// digits
// alpha (uppercase)
// alpha (lowercase)
// deno-fmt-ignore
const noEscapeAuth = new Int8Array([
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0x00 - 0x0F
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 0x10 - 0x1F
  0, 1, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 0, // 0x20 - 0x2F
  1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, // 0x30 - 0x3F
  0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0x40 - 0x4F
  1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, // 0x50 - 0x5F
  0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0x60 - 0x6F
  1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0,  // 0x70 - 0x7F
]);

// This prevents some common spoofing bugs due to our use of IDNA toASCII. For
// compatibility, the set of characters we use here is the *intersection* of
// "forbidden host code point" in the WHATWG URL Standard [1] and the
// characters in the host parsing loop in Url.prototype.parse, with the
// following additions:
//
// - ':' since this could cause a "protocol spoofing" bug
// - '@' since this could cause parts of the hostname to be confused with auth
// - '[' and ']' since this could cause a non-IPv6 hostname to be interpreted
//   as IPv6 by isIpv6Hostname above
//
// [1]: https://url.spec.whatwg.org/#forbidden-host-code-point
const forbiddenHostChars = /[\0\t\n\r #%/:<>?@[\\\]^|]/;
// For IPv6, permit '[', ']', and ':'.
const forbiddenHostCharsIpv6 = /[\0\t\n\r #%/<>?@\\^|]/;

const _url = URL;
export { _url as URL };

// Legacy URL API
export class Url {
  public protocol: string | null;
  public slashes: boolean | null;
  public auth: string | null;
  public host: string | null;
  public port: string | null;
  public hostname: string | null;
  public hash: string | null;
  public search: string | null;
  public query: string | ParsedUrlQuery | null;
  public pathname: string | null;
  public path: string | null;
  public href: string | null;
  [key: string]: unknown;

  constructor() {
    this.protocol = null;
    this.slashes = null;
    this.auth = null;
    this.host = null;
    this.port = null;
    this.hostname = null;
    this.hash = null;
    this.search = null;
    this.query = null;
    this.pathname = null;
    this.path = null;
    this.href = null;
  }

  #parseHost() {
    let host = this.host || "";
    let port: RegExpExecArray | null | string = portPattern.exec(host);
    if (port) {
      port = port[0];
      if (port !== ":") {
        this.port = port.slice(1);
      }
      host = host.slice(0, host.length - port.length);
    }
    if (host) this.hostname = host;
  }

  public resolve(relative: string) {
    return this.resolveObject(parse(relative, false, true)).format();
  }

  public resolveObject(relative: string | Url) {
    if (typeof relative === "string") {
      const rel = new Url();
      rel.urlParse(relative, false, true);
      relative = rel;
    }

    const result = new Url();
    const tkeys = Object.keys(this);
    for (let tk = 0; tk < tkeys.length; tk++) {
      const tkey = tkeys[tk];
      result[tkey] = this[tkey];
    }

    // Hash is always overridden, no matter what.
    // even href="" will remove it.
    result.hash = relative.hash;

    // If the relative url is empty, then there's nothing left to do here.
    if (relative.href === "") {
      result.href = result.format();
      return result;
    }

    // Hrefs like //foo/bar always cut to the protocol.
    if (relative.slashes && !relative.protocol) {
      // Take everything except the protocol from relative
      const rkeys = Object.keys(relative);
      for (let rk = 0; rk < rkeys.length; rk++) {
        const rkey = rkeys[rk];
        if (rkey !== "protocol") result[rkey] = relative[rkey];
      }

      // urlParse appends trailing / to urls like http://www.example.com
      if (
        result.protocol &&
        slashedProtocol.has(result.protocol) &&
        result.hostname &&
        !result.pathname
      ) {
        result.path = result.pathname = "/";
      }

      result.href = result.format();
      return result;
    }

    if (relative.protocol && relative.protocol !== result.protocol) {
      // If it's a known url protocol, then changing
      // the protocol does weird things
      // first, if it's not file:, then we MUST have a host,
      // and if there was a path
      // to begin with, then we MUST have a path.
      // if it is file:, then the host is dropped,
      // because that's known to be hostless.
      // anything else is assumed to be absolute.
      if (!slashedProtocol.has(relative.protocol)) {
        const keys = Object.keys(relative);
        for (let v = 0; v < keys.length; v++) {
          const k = keys[v];
          result[k] = relative[k];
        }
        result.href = result.format();
        return result;
      }

      result.protocol = relative.protocol;
      if (
        !relative.host &&
        !/^file:?$/.test(relative.protocol) &&
        !hostlessProtocol.has(relative.protocol)
      ) {
        const relPath = (relative.pathname || "").split("/");
        while (relPath.length && !(relative.host = relPath.shift() || null));
        if (!relative.host) relative.host = "";
        if (!relative.hostname) relative.hostname = "";
        if (relPath[0] !== "") relPath.unshift("");
        if (relPath.length < 2) relPath.unshift("");
        result.pathname = relPath.join("/");
      } else {
        result.pathname = relative.pathname;
      }
      result.search = relative.search;
      result.query = relative.query;
      result.host = relative.host || "";
      result.auth = relative.auth;
      result.hostname = relative.hostname || relative.host;
      result.port = relative.port;
      // To support http.request
      if (result.pathname || result.search) {
        const p = result.pathname || "";
        const s = result.search || "";
        result.path = p + s;
      }
      result.slashes = result.slashes || relative.slashes;
      result.href = result.format();
      return result;
    }

    const isSourceAbs = result.pathname && result.pathname.charAt(0) === "/";
    const isRelAbs = relative.host ||
      (relative.pathname && relative.pathname.charAt(0) === "/");
    let mustEndAbs: string | boolean | number | null = isRelAbs ||
      isSourceAbs || (result.host && relative.pathname);
    const removeAllDots = mustEndAbs;
    let srcPath = (result.pathname && result.pathname.split("/")) || [];
    const relPath = (relative.pathname && relative.pathname.split("/")) || [];
    const noLeadingSlashes = result.protocol &&
      !slashedProtocol.has(result.protocol);

    // If the url is a non-slashed url, then relative
    // links like ../.. should be able
    // to crawl up to the hostname, as well.  This is strange.
    // result.protocol has already been set by now.
    // Later on, put the first path part into the host field.
    if (noLeadingSlashes) {
      result.hostname = "";
      result.port = null;
      if (result.host) {
        if (srcPath[0] === "") srcPath[0] = result.host;
        else srcPath.unshift(result.host);
      }
      result.host = "";
      if (relative.protocol) {
        relative.hostname = null;
        relative.port = null;
        result.auth = null;
        if (relative.host) {
          if (relPath[0] === "") relPath[0] = relative.host;
          else relPath.unshift(relative.host);
        }
        relative.host = null;
      }
      mustEndAbs = mustEndAbs && (relPath[0] === "" || srcPath[0] === "");
    }

    if (isRelAbs) {
      // it's absolute.
      if (relative.host || relative.host === "") {
        if (result.host !== relative.host) result.auth = null;
        result.host = relative.host;
        result.port = relative.port;
      }
      if (relative.hostname || relative.hostname === "") {
        if (result.hostname !== relative.hostname) result.auth = null;
        result.hostname = relative.hostname;
      }
      result.search = relative.search;
      result.query = relative.query;
      srcPath = relPath;
      // Fall through to the dot-handling below.
    } else if (relPath.length) {
      // it's relative
      // throw away the existing file, and take the new path instead.
      if (!srcPath) srcPath = [];
      srcPath.pop();
      srcPath = srcPath.concat(relPath);
      result.search = relative.search;
      result.query = relative.query;
    } else if (relative.search !== null && relative.search !== undefined) {
      // Just pull out the search.
      // like href='?foo'.
      // Put this after the other two cases because it simplifies the booleans
      if (noLeadingSlashes) {
        result.hostname = result.host = srcPath.shift() || null;
        // Occasionally the auth can get stuck only in host.
        // This especially happens in cases like
        // url.resolveObject('mailto:local1@domain1', 'local2@domain2')
        const authInHost = result.host && result.host.indexOf("@") > 0 &&
          result.host.split("@");
        if (authInHost) {
          result.auth = authInHost.shift() || null;
          result.host = result.hostname = authInHost.shift() || null;
        }
      }
      result.search = relative.search;
      result.query = relative.query;
      // To support http.request
      if (result.pathname !== null || result.search !== null) {
        result.path = (result.pathname ? result.pathname : "") +
          (result.search ? result.search : "");
      }
      result.href = result.format();
      return result;
    }

    if (!srcPath.length) {
      // No path at all. All other things were already handled above.
      result.pathname = null;
      // To support http.request
      if (result.search) {
        result.path = "/" + result.search;
      } else {
        result.path = null;
      }
      result.href = result.format();
      return result;
    }

    // If a url ENDs in . or .., then it must get a trailing slash.
    // however, if it ends in anything else non-slashy,
    // then it must NOT get a trailing slash.
    let last = srcPath.slice(-1)[0];
    const hasTrailingSlash =
      ((result.host || relative.host || srcPath.length > 1) &&
        (last === "." || last === "..")) ||
      last === "";

    // Strip single dots, resolve double dots to parent dir
    // if the path tries to go above the root, `up` ends up > 0
    let up = 0;
    for (let i = srcPath.length - 1; i >= 0; i--) {
      last = srcPath[i];
      if (last === ".") {
        srcPath.splice(i, 1);
      } else if (last === "..") {
        srcPath.splice(i, 1);
        up++;
      } else if (up) {
        srcPath.splice(i, 1);
        up--;
      }
    }

    // If the path is allowed to go above the root, restore leading ..s
    if (!mustEndAbs && !removeAllDots) {
      while (up--) {
        srcPath.unshift("..");
      }
    }

    if (
      mustEndAbs &&
      srcPath[0] !== "" &&
      (!srcPath[0] || srcPath[0].charAt(0) !== "/")
    ) {
      srcPath.unshift("");
    }

    if (hasTrailingSlash && srcPath.join("/").slice(-1) !== "/") {
      srcPath.push("");
    }

    const isAbsolute = srcPath[0] === "" ||
      (srcPath[0] && srcPath[0].charAt(0) === "/");

    // put the host back
    if (noLeadingSlashes) {
      result.hostname = result.host = isAbsolute
        ? ""
        : srcPath.length
        ? srcPath.shift() || null
        : "";
      // Occasionally the auth can get stuck only in host.
      // This especially happens in cases like
      // url.resolveObject('mailto:local1@domain1', 'local2@domain2')
      const authInHost = result.host && result.host.indexOf("@") > 0
        ? result.host.split("@")
        : false;
      if (authInHost) {
        result.auth = authInHost.shift() || null;
        result.host = result.hostname = authInHost.shift() || null;
      }
    }

    mustEndAbs = mustEndAbs || (result.host && srcPath.length);

    if (mustEndAbs && !isAbsolute) {
      srcPath.unshift("");
    }

    if (!srcPath.length) {
      result.pathname = null;
      result.path = null;
    } else {
      result.pathname = srcPath.join("/");
    }

    // To support request.http
    if (result.pathname !== null || result.search !== null) {
      result.path = (result.pathname ? result.pathname : "") +
        (result.search ? result.search : "");
    }
    result.auth = relative.auth || result.auth;
    result.slashes = result.slashes || relative.slashes;
    result.href = result.format();
    return result;
  }

  format() {
    let auth = this.auth || "";
    if (auth) {
      auth = encodeStr(auth, noEscapeAuth, hexTable);
      auth += "@";
    }

    let protocol = this.protocol || "";
    let pathname = this.pathname || "";
    let hash = this.hash || "";
    let host = "";
    let query = "";

    if (this.host) {
      host = auth + this.host;
    } else if (this.hostname) {
      host = auth +
        (this.hostname.includes(":") && !isIpv6Hostname(this.hostname)
          ? "[" + this.hostname + "]"
          : this.hostname);
      if (this.port) {
        host += ":" + this.port;
      }
    }

    if (this.query !== null && typeof this.query === "object") {
      query = querystring.stringify(this.query);
    }

    let search = this.search || (query && "?" + query) || "";

    if (protocol && protocol.charCodeAt(protocol.length - 1) !== 58 /* : */) {
      protocol += ":";
    }

    let newPathname = "";
    let lastPos = 0;
    for (let i = 0; i < pathname.length; ++i) {
      switch (pathname.charCodeAt(i)) {
        case CHAR_HASH:
          if (i - lastPos > 0) {
            newPathname += pathname.slice(lastPos, i);
          }
          newPathname += "%23";
          lastPos = i + 1;
          break;
        case CHAR_QUESTION_MARK:
          if (i - lastPos > 0) {
            newPathname += pathname.slice(lastPos, i);
          }
          newPathname += "%3F";
          lastPos = i + 1;
          break;
      }
    }
    if (lastPos > 0) {
      if (lastPos !== pathname.length) {
        pathname = newPathname + pathname.slice(lastPos);
      } else pathname = newPathname;
    }

    // Only the slashedProtocols get the //.  Not mailto:, xmpp:, etc.
    // unless they had them to begin with.
    if (this.slashes || slashedProtocol.has(protocol)) {
      if (this.slashes || host) {
        if (pathname && pathname.charCodeAt(0) !== CHAR_FORWARD_SLASH) {
          pathname = "/" + pathname;
        }
        host = "//" + host;
      } else if (
        protocol.length >= 4 &&
        protocol.charCodeAt(0) === 102 /* f */ &&
        protocol.charCodeAt(1) === 105 /* i */ &&
        protocol.charCodeAt(2) === 108 /* l */ &&
        protocol.charCodeAt(3) === 101 /* e */
      ) {
        host = "//";
      }
    }

    search = search.replace(/#/g, "%23");

    if (hash && hash.charCodeAt(0) !== CHAR_HASH) {
      hash = "#" + hash;
    }
    if (search && search.charCodeAt(0) !== CHAR_QUESTION_MARK) {
      search = "?" + search;
    }

    return protocol + host + pathname + search + hash;
  }

  public urlParse(
    url: string,
    parseQueryString: boolean,
    slashesDenoteHost: boolean,
  ) {
    validateString(url, "url");

    // Copy chrome, IE, opera backslash-handling behavior.
    // Back slashes before the query string get converted to forward slashes
    // See: https://code.google.com/p/chromium/issues/detail?id=25916
    let hasHash = false;
    let start = -1;
    let end = -1;
    let rest = "";
    let lastPos = 0;
    for (let i = 0, inWs = false, split = false; i < url.length; ++i) {
      const code = url.charCodeAt(i);

      // Find first and last non-whitespace characters for trimming
      const isWs = code === CHAR_SPACE ||
        code === CHAR_TAB ||
        code === CHAR_CARRIAGE_RETURN ||
        code === CHAR_LINE_FEED ||
        code === CHAR_FORM_FEED ||
        code === CHAR_NO_BREAK_SPACE ||
        code === CHAR_ZERO_WIDTH_NOBREAK_SPACE;
      if (start === -1) {
        if (isWs) continue;
        lastPos = start = i;
      } else if (inWs) {
        if (!isWs) {
          end = -1;
          inWs = false;
        }
      } else if (isWs) {
        end = i;
        inWs = true;
      }

      // Only convert backslashes while we haven't seen a split character
      if (!split) {
        switch (code) {
          case CHAR_HASH:
            hasHash = true;
          // Fall through
          case CHAR_QUESTION_MARK:
            split = true;
            break;
          case CHAR_BACKWARD_SLASH:
            if (i - lastPos > 0) rest += url.slice(lastPos, i);
            rest += "/";
            lastPos = i + 1;
            break;
        }
      } else if (!hasHash && code === CHAR_HASH) {
        hasHash = true;
      }
    }

    // Check if string was non-empty (including strings with only whitespace)
    if (start !== -1) {
      if (lastPos === start) {
        // We didn't convert any backslashes

        if (end === -1) {
          if (start === 0) rest = url;
          else rest = url.slice(start);
        } else {
          rest = url.slice(start, end);
        }
      } else if (end === -1 && lastPos < url.length) {
        // We converted some backslashes and have only part of the entire string
        rest += url.slice(lastPos);
      } else if (end !== -1 && lastPos < end) {
        // We converted some backslashes and have only part of the entire string
        rest += url.slice(lastPos, end);
      }
    }

    if (!slashesDenoteHost && !hasHash) {
      // Try fast path regexp
      const simplePath = simplePathPattern.exec(rest);
      if (simplePath) {
        this.path = rest;
        this.href = rest;
        this.pathname = simplePath[1];
        if (simplePath[2]) {
          this.search = simplePath[2];
          if (parseQueryString) {
            this.query = querystring.parse(this.search.slice(1));
          } else {
            this.query = this.search.slice(1);
          }
        } else if (parseQueryString) {
          this.search = null;
          this.query = Object.create(null);
        }
        return this;
      }
    }

    let proto: RegExpExecArray | null | string = protocolPattern.exec(rest);
    let lowerProto = "";
    if (proto) {
      proto = proto[0];
      lowerProto = proto.toLowerCase();
      this.protocol = lowerProto;
      rest = rest.slice(proto.length);
    }

    // Figure out if it's got a host
    // user@server is *always* interpreted as a hostname, and url
    // resolution will treat //foo/bar as host=foo,path=bar because that's
    // how the browser resolves relative URLs.
    let slashes;
    if (slashesDenoteHost || proto || hostPattern.test(rest)) {
      slashes = rest.charCodeAt(0) === CHAR_FORWARD_SLASH &&
        rest.charCodeAt(1) === CHAR_FORWARD_SLASH;
      if (slashes && !(proto && hostlessProtocol.has(lowerProto))) {
        rest = rest.slice(2);
        this.slashes = true;
      }
    }

    if (
      !hostlessProtocol.has(lowerProto) &&
      (slashes || (proto && !slashedProtocol.has(proto)))
    ) {
      // there's a hostname.
      // the first instance of /, ?, ;, or # ends the host.
      //
      // If there is an @ in the hostname, then non-host chars *are* allowed
      // to the left of the last @ sign, unless some host-ending character
      // comes *before* the @-sign.
      // URLs are obnoxious.
      //
      // ex:
      // http://a@b@c/ => user:a@b host:c
      // http://a@b?@c => user:a host:b path:/?@c

      let hostEnd = -1;
      let atSign = -1;
      let nonHost = -1;
      for (let i = 0; i < rest.length; ++i) {
        switch (rest.charCodeAt(i)) {
          case CHAR_TAB:
          case CHAR_LINE_FEED:
          case CHAR_CARRIAGE_RETURN:
          case CHAR_SPACE:
          case CHAR_DOUBLE_QUOTE:
          case CHAR_PERCENT:
          case CHAR_SINGLE_QUOTE:
          case CHAR_SEMICOLON:
          case CHAR_LEFT_ANGLE_BRACKET:
          case CHAR_RIGHT_ANGLE_BRACKET:
          case CHAR_BACKWARD_SLASH:
          case CHAR_CIRCUMFLEX_ACCENT:
          case CHAR_GRAVE_ACCENT:
          case CHAR_LEFT_CURLY_BRACKET:
          case CHAR_VERTICAL_LINE:
          case CHAR_RIGHT_CURLY_BRACKET:
            // Characters that are never ever allowed in a hostname from RFC 2396
            if (nonHost === -1) nonHost = i;
            break;
          case CHAR_HASH:
          case CHAR_FORWARD_SLASH:
          case CHAR_QUESTION_MARK:
            // Find the first instance of any host-ending characters
            if (nonHost === -1) nonHost = i;
            hostEnd = i;
            break;
          case CHAR_AT:
            // At this point, either we have an explicit point where the
            // auth portion cannot go past, or the last @ char is the decider.
            atSign = i;
            nonHost = -1;
            break;
        }
        if (hostEnd !== -1) break;
      }
      start = 0;
      if (atSign !== -1) {
        this.auth = decodeURIComponent(rest.slice(0, atSign));
        start = atSign + 1;
      }
      if (nonHost === -1) {
        this.host = rest.slice(start);
        rest = "";
      } else {
        this.host = rest.slice(start, nonHost);
        rest = rest.slice(nonHost);
      }

      // pull out port.
      this.#parseHost();

      // We've indicated that there is a hostname,
      // so even if it's empty, it has to be present.
      if (typeof this.hostname !== "string") this.hostname = "";

      const hostname = this.hostname;

      // If hostname begins with [ and ends with ]
      // assume that it's an IPv6 address.
      const ipv6Hostname = isIpv6Hostname(hostname);

      // validate a little.
      if (!ipv6Hostname) {
        rest = getHostname(this, rest, hostname);
      }

      if (this.hostname.length > hostnameMaxLen) {
        this.hostname = "";
      } else {
        // Hostnames are always lower case.
        this.hostname = this.hostname.toLowerCase();
      }

      if (this.hostname !== "") {
        if (ipv6Hostname) {
          if (forbiddenHostCharsIpv6.test(this.hostname)) {
            throw new ERR_INVALID_URL(url);
          }
        } else {
          // IDNA Support: Returns a punycoded representation of "domain".
          // It only converts parts of the domain name that
          // have non-ASCII characters, i.e. it doesn't matter if
          // you call it with a domain that already is ASCII-only.

          // Use lenient mode (`true`) to try to support even non-compliant
          // URLs.
          this.hostname = idnaToASCII(this.hostname);

          // Prevent two potential routes of hostname spoofing.
          // 1. If this.hostname is empty, it must have become empty due to toASCII
          //    since we checked this.hostname above.
          // 2. If any of forbiddenHostChars appears in this.hostname, it must have
          //    also gotten in due to toASCII. This is since getHostname would have
          //    filtered them out otherwise.
          // Rather than trying to correct this by moving the non-host part into
          // the pathname as we've done in getHostname, throw an exception to
          // convey the severity of this issue.
          if (this.hostname === "" || forbiddenHostChars.test(this.hostname)) {
            throw new ERR_INVALID_URL(url);
          }
        }
      }

      const p = this.port ? ":" + this.port : "";
      const h = this.hostname || "";
      this.host = h + p;

      // strip [ and ] from the hostname
      // the host field still retains them, though
      if (ipv6Hostname) {
        this.hostname = this.hostname.slice(1, -1);
        if (rest[0] !== "/") {
          rest = "/" + rest;
        }
      }
    }

    // Now rest is set to the post-host stuff.
    // Chop off any delim chars.
    if (!unsafeProtocol.has(lowerProto)) {
      // First, make 100% sure that any "autoEscape" chars get
      // escaped, even if encodeURIComponent doesn't think they
      // need to be.
      rest = autoEscapeStr(rest);
    }

    let questionIdx = -1;
    let hashIdx = -1;
    for (let i = 0; i < rest.length; ++i) {
      const code = rest.charCodeAt(i);
      if (code === CHAR_HASH) {
        this.hash = rest.slice(i);
        hashIdx = i;
        break;
      } else if (code === CHAR_QUESTION_MARK && questionIdx === -1) {
        questionIdx = i;
      }
    }

    if (questionIdx !== -1) {
      if (hashIdx === -1) {
        this.search = rest.slice(questionIdx);
        this.query = rest.slice(questionIdx + 1);
      } else {
        this.search = rest.slice(questionIdx, hashIdx);
        this.query = rest.slice(questionIdx + 1, hashIdx);
      }
      if (parseQueryString) {
        this.query = querystring.parse(this.query);
      }
    } else if (parseQueryString) {
      // No query string, but parseQueryString still requested
      this.search = null;
      this.query = Object.create(null);
    }

    const useQuestionIdx = questionIdx !== -1 &&
      (hashIdx === -1 || questionIdx < hashIdx);
    const firstIdx = useQuestionIdx ? questionIdx : hashIdx;
    if (firstIdx === -1) {
      if (rest.length > 0) this.pathname = rest;
    } else if (firstIdx > 0) {
      this.pathname = rest.slice(0, firstIdx);
    }
    if (slashedProtocol.has(lowerProto) && this.hostname && !this.pathname) {
      this.pathname = "/";
    }

    // To support http.request
    if (this.pathname || this.search) {
      const p = this.pathname || "";
      const s = this.search || "";
      this.path = p + s;
    }

    // Finally, reconstruct the href based on what has been validated.
    this.href = this.format();
    return this;
  }
}

interface UrlObject {
  auth?: string | null | undefined;
  hash?: string | null | undefined;
  host?: string | null | undefined;
  hostname?: string | null | undefined;
  href?: string | null | undefined;
  pathname?: string | null | undefined;
  protocol?: string | null | undefined;
  search?: string | null | undefined;
  slashes?: boolean | null | undefined;
  port?: string | number | null | undefined;
  query?: string | null | ParsedUrlQueryInput | undefined;
}

export function format(
  urlObject: string | URL | Url | UrlObject,
  options?: {
    auth: boolean;
    fragment: boolean;
    search: boolean;
    unicode: boolean;
  },
): string {
  if (typeof urlObject === "string") {
    urlObject = parse(urlObject, true, false);
  } else if (typeof urlObject !== "object" || urlObject === null) {
    throw new ERR_INVALID_ARG_TYPE(
      "urlObject",
      ["Object", "string"],
      urlObject,
    );
  } else if (urlObject instanceof URL) {
    return formatWhatwg(urlObject, options);
  }

  return Url.prototype.format.call(urlObject);
}

/**
 * The URL object has both a `toString()` method and `href` property that return string serializations of the URL.
 * These are not, however, customizable in any way.
 * This method allows for basic customization of the output.
 * @see Tested in `parallel/test-url-format-whatwg.js`.
 * @param urlObject
 * @param options
 * @param options.auth `true` if the serialized URL string should include the username and password, `false` otherwise. **Default**: `true`.
 * @param options.fragment `true` if the serialized URL string should include the fragment, `false` otherwise. **Default**: `true`.
 * @param options.search `true` if the serialized URL string should include the search query, **Default**: `true`.
 * @param options.unicode `true` if Unicode characters appearing in the host component of the URL string should be encoded directly as opposed to being Punycode encoded. **Default**: `false`.
 * @returns a customizable serialization of a URL `String` representation of a `WHATWG URL` object.
 */
function formatWhatwg(
  urlObject: string | URL,
  options?: {
    auth: boolean;
    fragment: boolean;
    search: boolean;
    unicode: boolean;
  },
): string {
  if (typeof urlObject === "string") {
    urlObject = new URL(urlObject);
  }
  if (options) {
    if (typeof options !== "object") {
      throw new ERR_INVALID_ARG_TYPE("options", "object", options);
    }
  }

  options = {
    auth: true,
    fragment: true,
    search: true,
    unicode: false,
    ...options,
  };

  let ret = urlObject.protocol;
  if (urlObject.host !== null) {
    ret += "//";
    const hasUsername = !!urlObject.username;
    const hasPassword = !!urlObject.password;
    if (options.auth && (hasUsername || hasPassword)) {
      if (hasUsername) {
        ret += urlObject.username;
      }
      if (hasPassword) {
        ret += `:${urlObject.password}`;
      }
      ret += "@";
    }
    ret += options.unicode
      ? domainToUnicode(urlObject.hostname)
      : urlObject.hostname;
    if (urlObject.port) {
      ret += `:${urlObject.port}`;
    }
  }

  ret += urlObject.pathname;

  if (options.search && urlObject.search) {
    ret += urlObject.search;
  }
  if (options.fragment && urlObject.hash) {
    ret += urlObject.hash;
  }

  return ret;
}

function isIpv6Hostname(hostname: string) {
  return (
    hostname.charCodeAt(0) === CHAR_LEFT_SQUARE_BRACKET &&
    hostname.charCodeAt(hostname.length - 1) === CHAR_RIGHT_SQUARE_BRACKET
  );
}

function getHostname(self: Url, rest: string, hostname: string) {
  for (let i = 0; i < hostname.length; ++i) {
    const code = hostname.charCodeAt(i);
    const isValid = (code >= CHAR_LOWERCASE_A && code <= CHAR_LOWERCASE_Z) ||
      code === CHAR_DOT ||
      (code >= CHAR_UPPERCASE_A && code <= CHAR_UPPERCASE_Z) ||
      (code >= CHAR_0 && code <= CHAR_9) ||
      code === CHAR_HYPHEN_MINUS ||
      code === CHAR_PLUS ||
      code === CHAR_UNDERSCORE ||
      code > 127;

    // Invalid host character
    if (!isValid) {
      self.hostname = hostname.slice(0, i);
      return `/${hostname.slice(i)}${rest}`;
    }
  }
  return rest;
}

// Escaped characters. Use empty strings to fill up unused entries.
// Using Array is faster than Object/Map
// deno-fmt-ignore
const escapedCodes = [
  /* 0 - 9 */ "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "%09",
  /* 10 - 19 */ "%0A",
  "",
  "",
  "%0D",
  "",
  "",
  "",
  "",
  "",
  "",
  /* 20 - 29 */ "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  /* 30 - 39 */ "",
  "",
  "%20",
  "",
  "%22",
  "",
  "",
  "",
  "",
  "%27",
  /* 40 - 49 */ "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  /* 50 - 59 */ "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  /* 60 - 69 */ "%3C",
  "",
  "%3E",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  /* 70 - 79 */ "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  /* 80 - 89 */ "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  /* 90 - 99 */ "",
  "",
  "%5C",
  "",
  "%5E",
  "",
  "%60",
  "",
  "",
  "",
  /* 100 - 109 */ "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  /* 110 - 119 */ "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  "",
  /* 120 - 125 */ "",
  "",
  "",
  "%7B",
  "%7C",
  "%7D"
];

// Automatically escape all delimiters and unwise characters from RFC 2396.
// Also escape single quotes in case of an XSS attack.
// Return the escaped string.
function autoEscapeStr(rest: string) {
  let escaped = "";
  let lastEscapedPos = 0;
  for (let i = 0; i < rest.length; ++i) {
    // `escaped` contains substring up to the last escaped character.
    const escapedChar = escapedCodes[rest.charCodeAt(i)];
    if (escapedChar) {
      // Concat if there are ordinary characters in the middle.
      if (i > lastEscapedPos) {
        escaped += rest.slice(lastEscapedPos, i);
      }
      escaped += escapedChar;
      lastEscapedPos = i + 1;
    }
  }
  if (lastEscapedPos === 0) {
    // Nothing has been escaped.
    return rest;
  }

  // There are ordinary characters at the end.
  if (lastEscapedPos < rest.length) {
    escaped += rest.slice(lastEscapedPos);
  }

  return escaped;
}

/**
 * The url.urlParse() method takes a URL string, parses it, and returns a URL object.
 *
 * @see Tested in `parallel/test-url-parse-format.js`.
 * @param url The URL string to parse.
 * @param parseQueryString If `true`, the query property will always be set to an object returned by the querystring module's parse() method. If false,
 * the query property on the returned URL object will be an unparsed, undecoded string. Default: false.
 * @param slashesDenoteHost If `true`, the first token after the literal string // and preceding the next / will be interpreted as the host
 */
export function parse(
  url: string | Url,
  parseQueryString: boolean,
  slashesDenoteHost: boolean,
) {
  if (url instanceof Url) return url;

  const urlObject = new Url();
  urlObject.urlParse(url, parseQueryString, slashesDenoteHost);
  return urlObject;
}

/** The url.resolve() method resolves a target URL relative to a base URL in a manner similar to that of a Web browser resolving an anchor tag HREF.
 * @see https://nodejs.org/api/url.html#urlresolvefrom-to
 * @legacy
 */
export function resolve(from: string, to: string) {
  return parse(from, false, true).resolve(to);
}

export function resolveObject(source: string | Url, relative: string) {
  if (!source) return relative;
  return parse(source, false, true).resolveObject(relative);
}

/**
 * The url.domainToASCII() takes an arbitrary domain and attempts to convert it into an IDN
 *
 * @param domain The domain to convert to an IDN
 * @see https://www.rfc-editor.org/rfc/rfc3490#section-4
 */
export function domainToASCII(domain: string) {
  return idnaToASCII(domain);
}

/**
 * The url.domainToUnicode() takes an IDN and attempts to convert it into unicode
 *
 * @param domain The IDN to convert to Unicode
 * @see https://www.rfc-editor.org/rfc/rfc3490#section-4
 */
export function domainToUnicode(domain: string) {
  return idnaToUnicode(domain);
}

/**
 * This function ensures the correct decodings of percent-encoded characters as well as ensuring a cross-platform valid absolute path string.
 * @see Tested in `parallel/test-fileurltopath.js`.
 * @param path The file URL string or URL object to convert to a path.
 * @returns The fully-resolved platform-specific Node.js file path.
 */
export function fileURLToPath(path: string | URL): string {
  if (typeof path === "string") path = new URL(path);
  else if (!(path instanceof URL)) {
    throw new ERR_INVALID_ARG_TYPE("path", ["string", "URL"], path);
  }
  if (path.protocol !== "file:") {
    throw new ERR_INVALID_URL_SCHEME("file");
  }
  return isWindows ? getPathFromURLWin(path) : getPathFromURLPosix(path);
}

function getPathFromURLWin(url: URL): string {
  const hostname = url.hostname;
  let pathname = url.pathname;
  for (let n = 0; n < pathname.length; n++) {
    if (pathname[n] === "%") {
      const third = pathname.codePointAt(n + 2)! | 0x20;
      if (
        (pathname[n + 1] === "2" && third === 102) || // 2f 2F /
        (pathname[n + 1] === "5" && third === 99) // 5c 5C \
      ) {
        throw new ERR_INVALID_FILE_URL_PATH(
          "must not include encoded \\ or / characters",
        );
      }
    }
  }

  pathname = pathname.replace(forwardSlashRegEx, "\\");
  pathname = decodeURIComponent(pathname);
  if (hostname !== "") {
    // TODO(bartlomieju): add support for punycode encodings
    return `\\\\${hostname}${pathname}`;
  } else {
    // Otherwise, it's a local path that requires a drive letter
    const letter = pathname.codePointAt(1)! | 0x20;
    const sep = pathname[2];
    if (
      letter < CHAR_LOWERCASE_A ||
      letter > CHAR_LOWERCASE_Z || // a..z A..Z
      sep !== ":"
    ) {
      throw new ERR_INVALID_FILE_URL_PATH("must be absolute");
    }
    return pathname.slice(1);
  }
}

function getPathFromURLPosix(url: URL): string {
  if (url.hostname !== "") {
    throw new ERR_INVALID_FILE_URL_HOST(osType);
  }
  const pathname = url.pathname;
  for (let n = 0; n < pathname.length; n++) {
    if (pathname[n] === "%") {
      const third = pathname.codePointAt(n + 2)! | 0x20;
      if (pathname[n + 1] === "2" && third === 102) {
        throw new ERR_INVALID_FILE_URL_PATH(
          "must not include encoded / characters",
        );
      }
    }
  }
  return decodeURIComponent(pathname);
}

/**
 *  The following characters are percent-encoded when converting from file path
 *  to URL:
 *  - %: The percent character is the only character not encoded by the
 *       `pathname` setter.
 *  - \: Backslash is encoded on non-windows platforms since it's a valid
 *       character but the `pathname` setters replaces it by a forward slash.
 *  - LF: The newline character is stripped out by the `pathname` setter.
 *        (See whatwg/url#419)
 *  - CR: The carriage return character is also stripped out by the `pathname`
 *        setter.
 *  - TAB: The tab character is also stripped out by the `pathname` setter.
 */
function encodePathChars(
  filepath: string,
  options: { windows?: boolean },
): string {
  const windows = options.windows;
  if (filepath.includes("%")) {
    filepath = filepath.replace(percentRegEx, "%25");
  }
  // In posix, backslash is a valid character in paths:
  if (!(windows ?? isWindows) && filepath.includes("\\")) {
    filepath = filepath.replace(backslashRegEx, "%5C");
  }
  if (filepath.includes("\n")) {
    filepath = filepath.replace(newlineRegEx, "%0A");
  }
  if (filepath.includes("\r")) {
    filepath = filepath.replace(carriageReturnRegEx, "%0D");
  }
  if (filepath.includes("\t")) {
    filepath = filepath.replace(tabRegEx, "%09");
  }
  return filepath;
}

/**
 * This function ensures that `filepath` is resolved absolutely, and that the URL control characters are correctly encoded when converting into a File URL.
 * @see Tested in `parallel/test-url-pathtofileurl.js`.
 * @param filepath The file path string to convert to a file URL.
 * @param options The options.
 * @returns The file URL object.
 */
export function pathToFileURL(
  filepath: string,
  options: { windows?: boolean } = {},
): URL {
  validateString(filepath, "path");
  const windows = options?.windows;
  const outURL = new URL("file://");
  if ((windows ?? isWindows) && filepath.startsWith("\\\\")) {
    // UNC path format: \\server\share\resource
    const paths = filepath.split("\\");
    if (paths.length <= 3) {
      throw new ERR_INVALID_ARG_VALUE(
        "filepath",
        filepath,
        "Missing UNC resource path",
      );
    }
    const hostname = paths[2];
    if (hostname.length === 0) {
      throw new ERR_INVALID_ARG_VALUE(
        "filepath",
        filepath,
        "Empty UNC servername",
      );
    }

    outURL.hostname = idnaToASCII(hostname);
    outURL.pathname = encodePathChars(paths.slice(3).join("/"), { windows });
  } else {
    let resolved = (windows ?? isWindows)
      ? path.win32.resolve(filepath)
      : path.posix.resolve(filepath);
    // path.resolve strips trailing slashes so we must add them back
    const filePathLast = filepath.charCodeAt(filepath.length - 1);
    if (
      (filePathLast === CHAR_FORWARD_SLASH ||
        ((windows ?? isWindows) && filePathLast === CHAR_BACKWARD_SLASH)) &&
      resolved[resolved.length - 1] !== path.sep
    ) {
      resolved += "/";
    }

    outURL.pathname = encodePathChars(resolved, { windows });
  }
  return outURL;
}

interface HttpOptions {
  protocol: string;
  hostname: string;
  hash: string;
  search: string;
  pathname: string;
  path: string;
  href: string;
  port?: number;
  auth?: string;
}

/**
 * This utility function converts a URL object into an ordinary options object as expected by the `http.request()` and `https.request()` APIs.
 * @see Tested in `parallel/test-url-urltooptions.js`.
 * @param url The `WHATWG URL` object to convert to an options object.
 * @returns HttpOptions
 * @returns HttpOptions.protocol Protocol to use.
 * @returns HttpOptions.hostname A domain name or IP address of the server to issue the request to.
 * @returns HttpOptions.hash The fragment portion of the URL.
 * @returns HttpOptions.search The serialized query portion of the URL.
 * @returns HttpOptions.pathname The path portion of the URL.
 * @returns HttpOptions.path Request path. Should include query string if any. E.G. `'/index.html?page=12'`. An exception is thrown when the request path contains illegal characters. Currently, only spaces are rejected but that may change in the future.
 * @returns HttpOptions.href The serialized URL.
 * @returns HttpOptions.port Port of remote server.
 * @returns HttpOptions.auth Basic authentication i.e. `'user:password'` to compute an Authorization header.
 */
export function urlToHttpOptions(url: URL): HttpOptions {
  const options: HttpOptions = {
    protocol: url.protocol,
    hostname: typeof url.hostname === "string" && url.hostname.startsWith("[")
      ? url.hostname.slice(1, -1)
      : url.hostname,
    hash: url.hash,
    search: url.search,
    pathname: url.pathname,
    path: `${url.pathname || ""}${url.search || ""}`,
    href: url.href,
  };
  if (url.port !== "") {
    options.port = Number(url.port);
  }
  if (url.username || url.password) {
    options.auth = `${decodeURIComponent(url.username)}:${
      decodeURIComponent(
        url.password,
      )
    }`;
  }
  return options;
}

const URLSearchParams_ = URLSearchParams;
export { URLSearchParams_ as URLSearchParams };

export default {
  parse,
  format,
  resolve,
  resolveObject,
  domainToASCII,
  domainToUnicode,
  fileURLToPath,
  pathToFileURL,
  urlToHttpOptions,
  Url,
  URL,
  URLSearchParams,
};
