import {
  CHAR_SPACE,
  CHAR_TAB,
  CHAR_CARRIAGE_RETURN,
  CHAR_LINE_FEED,
  CHAR_FORM_FEED,
  CHAR_NO_BREAK_SPACE,
  CHAR_ZERO_WIDTH_NOBREAK_SPACE,
  CHAR_HASH,
  CHAR_FORWARD_SLASH,
  CHAR_LEFT_SQUARE_BRACKET,
  CHAR_RIGHT_SQUARE_BRACKET,
  CHAR_LEFT_ANGLE_BRACKET,
  CHAR_RIGHT_ANGLE_BRACKET,
  CHAR_LEFT_CURLY_BRACKET,
  CHAR_RIGHT_CURLY_BRACKET,
  CHAR_QUESTION_MARK,
  CHAR_LOWERCASE_A,
  CHAR_LOWERCASE_Z,
  CHAR_UPPERCASE_A,
  CHAR_UPPERCASE_Z,
  CHAR_DOT,
  CHAR_0,
  CHAR_9,
  CHAR_HYPHEN_MINUS,
  CHAR_PLUS,
  CHAR_UNDERSCORE,
  CHAR_DOUBLE_QUOTE,
  CHAR_SINGLE_QUOTE,
  CHAR_PERCENT,
  CHAR_SEMICOLON,
  CHAR_BACKWARD_SLASH,
  CHAR_CIRCUMFLEX_ACCENT,
  CHAR_GRAVE_ACCENT,
  CHAR_VERTICAL_LINE,
  CHAR_AT,
} from "../path/constants.ts";

const formatSymbol = Symbol("format");

import { spliceOne } from "./_utils.ts";
import * as querystring from "./querystring.ts";
import * as path from "./path.ts";

const { build } = Deno;
const isWindows = build.os === "win";

//TODO convert Set to SafeSet
const hostlessProtocol = new Set(["javascript", "javascript:"]);

// Protocols that can allow "unsafe" and "unwise" chars.
const unsafeProtocol = new Set(["javascript", "javascript:"]);

//TODO convert Set to SafeSet
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

const simplePathPattern = /^(\/\/?(?!\/)[^?\s]*)(\?[^\s]*)?$/;
const portPattern = /:[0-9]*$/;
const protocolPattern = /^[a-z0-9.+-]+:/i;
const hostPattern = /^\/\/[^@/]+@[^@/]+/;
const forwardSlashRegEx = /\//g;
const percentRegEx = /%/g;
const backslashRegEx = /\\/g;
const newlineRegEx = /\n/g;
const carriageReturnRegEx = /\r/g;
const tabRegEx = /\t/g;
const hostnameMaxLen = 255;

export class Url {
  protocol: string | null = null;
  slashes: boolean | null = null;
  auth: string | null | undefined = null;
  host: string | null | undefined = null;
  port: string | null = null;
  hostname: string | null | undefined = null;
  hash: string | null = null;
  search: string | null = null;
  query: object | string | null = null;
  pathname: string | null = null;
  path: string | null = null;
  href: string | null = null;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [key: string]: any;
  parse(
    url: string,
    parseQueryString: boolean,
    slashesDenoteHost: boolean
  ): Url {
    let hasHash = false;
    let start = -1;
    let end = -1;
    let rest = "";
    let lastPos = 0;

    for (let i = 0, inWs = false, split = false; i < url.length; ++i) {
      const code = url.charCodeAt(i);
      const isWs =
        code === CHAR_SPACE ||
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

      if (!split) {
        switch (code) {
          case CHAR_HASH:
            hasHash = true;
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

    if (start !== -1) {
      if (lastPos === start) {
        if (end === -1) {
          if (start === 0) rest = url;
          else rest = url.slice(start);
        } else {
          rest = url.slice(start, end);
        }
      } else if (end === -1 && lastPos < url.length) {
        rest += url.slice(lastPos);
      } else if (end !== -1 && lastPos < end) {
        rest += url.slice(lastPos, end);
      }
    }

    if (!slashesDenoteHost && !hasHash) {
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
          this.query = Object.create({});
        }
        return this;
      }
    }

    const protoRegex = protocolPattern.exec(rest);
    let proto;
    let lowerProto;
    if (protoRegex) {
      proto = protoRegex[0];
      lowerProto = proto.toLowerCase();
      this.protocol = lowerProto;
      rest = rest.slice(proto.length);
    }

    let slashes;
    if (slashesDenoteHost || proto || hostPattern.test(rest)) {
      slashes =
        rest.charCodeAt(0) === CHAR_FORWARD_SLASH &&
        rest.charCodeAt(1) === CHAR_FORWARD_SLASH;
      if (
        slashes &&
        !(proto && lowerProto && hostlessProtocol.has(lowerProto))
      ) {
        rest = rest.slice(2);
        this.slashes = true;
      }
    }

    if (
      lowerProto &&
      !hostlessProtocol.has(lowerProto) &&
      (slashes || (proto && !slashedProtocol.has(proto)))
    ) {
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
            if (nonHost === -1) nonHost = i;
            break;
          case CHAR_HASH:
          case CHAR_FORWARD_SLASH:
          case CHAR_QUESTION_MARK:
            if (nonHost === -1) nonHost = i;
            hostEnd = i;
            break;
          case CHAR_AT:
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

      this.parseHost();

      if (typeof this.hostname !== "string") this.hostname = "";

      const hostname = this.hostname;

      const ipv6Hostname =
        hostname.charCodeAt(0) === CHAR_LEFT_SQUARE_BRACKET &&
        hostname.charCodeAt(hostname.length - 1) === CHAR_RIGHT_SQUARE_BRACKET;

      if (!ipv6Hostname) {
        rest = getHostname(this, rest, hostname);
      }

      if (this.hostname.length > hostnameMaxLen) {
        this.hostname = "";
      } else {
        this.hostname = this.hostname.toLowerCase();
      }

      //TODO : convert this.hostname to a punycoded representation of "domain".

      const p = this.port ? ":" + this.port : "";
      const h = this.hostname || "";
      this.host = h + p;

      if (ipv6Hostname) {
        this.hostname = this.hostname.slice(1, -1);
        if (rest[0] !== "/") {
          rest = "/" + rest;
        }
      }
    }
    if (lowerProto && !unsafeProtocol.has(lowerProto)) {
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
      this.search = null;
      this.query = Object.create({});
    }

    const useQuestionIdx =
      questionIdx !== -1 && (hashIdx === -1 || questionIdx < hashIdx);
    const firstIdx = useQuestionIdx ? questionIdx : hashIdx;
    if (firstIdx === -1) {
      if (rest.length > 0) this.pathname = rest;
    } else if (firstIdx > 0) {
      this.pathname = rest.slice(0, firstIdx);
    }
    if (
      lowerProto &&
      slashedProtocol.has(lowerProto) &&
      this.hostname &&
      !this.pathname
    ) {
      this.pathname = "/";
    }

    if (this.pathname || this.search) {
      const p = this.pathname || "";
      const s = this.search || "";
      this.path = p + s;
    }

    this.href = this.format();
    return this;
  }
  parseHost(): void {
    let host = this.host || "";
    const portRegex = portPattern.exec(host);
    let port;
    if (portRegex) {
      port = portRegex[0];
      if (port !== ":") {
        this.port = port.slice(1);
      }
      host = host.slice(0, host.length - port.length);
    }
    if (host) this.hostname = host;
  }
  format(): string {
    let auth = this.auth || "";
    if (auth) {
      auth = querystring.encodeStr(auth, noEscapeAuth, querystring.hexTable);
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
      host =
        auth +
        (this.hostname.includes(":")
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

    if (protocol && protocol.charCodeAt(protocol.length - 1) !== 58 /* : */)
      protocol += ":";

    let newPathname = "";
    let lastPos = 0;
    for (let i = 0; i < pathname.length; ++i) {
      switch (pathname.charCodeAt(i)) {
        case CHAR_HASH:
          if (i - lastPos > 0) newPathname += pathname.slice(lastPos, i);
          newPathname += "%23";
          lastPos = i + 1;
          break;
        case CHAR_QUESTION_MARK:
          if (i - lastPos > 0) newPathname += pathname.slice(lastPos, i);
          newPathname += "%3F";
          lastPos = i + 1;
          break;
      }
    }
    if (lastPos > 0) {
      if (lastPos !== pathname.length)
        pathname = newPathname + pathname.slice(lastPos);
      else pathname = newPathname;
    }

    // Only the slashedProtocols get the //.  Not mailto:, xmpp:, etc.
    // unless they had them to begin with.
    if (this.slashes || slashedProtocol.has(protocol)) {
      if (this.slashes || host) {
        if (pathname && pathname.charCodeAt(0) !== CHAR_FORWARD_SLASH)
          pathname = "/" + pathname;
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

    if (hash && hash.charCodeAt(0) !== CHAR_HASH) hash = "#" + hash;
    if (search && search.charCodeAt(0) !== CHAR_QUESTION_MARK)
      search = "?" + search;

    return protocol + host + pathname + search + hash;
  }
  resolveObject(relative: string | Url): Url {
    if (typeof relative === "string") {
      const rel = new Url();
      rel.parse(relative, false, true);
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
        while (relPath.length && !(relative.host = relPath.shift()));
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
    const isRelAbs =
      relative.host ||
      (relative.pathname && relative.pathname.charAt(0) === "/");
    let mustEndAbs: string | undefined | boolean | null | number =
      isRelAbs || isSourceAbs || (result.host && relative.pathname);
    const removeAllDots = mustEndAbs;
    let srcPath = (result.pathname && result.pathname.split("/")) || [];
    const relPath = (relative.pathname && relative.pathname.split("/")) || [];
    const noLeadingSlashes =
      result.protocol && !slashedProtocol.has(result.protocol);

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
        result.hostname = result.host = srcPath.shift();
        // Occasionally the auth can get stuck only in host.
        // This especially happens in cases like
        // url.resolveObject('mailto:local1@domain1', 'local2@domain2')
        const authInHost =
          result.host && result.host.indexOf("@") > 0 && result.host.split("@");
        if (authInHost) {
          result.auth = authInHost.shift();
          result.host = result.hostname = authInHost.shift();
        }
      }
      result.search = relative.search;
      result.query = relative.query;
      // To support http.request
      if (result.pathname !== null || result.search !== null) {
        result.path =
          (result.pathname ? result.pathname : "") +
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
        spliceOne(srcPath, i);
      } else if (last === "..") {
        spliceOne(srcPath, i);
        up++;
      } else if (up) {
        spliceOne(srcPath, i);
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

    if (hasTrailingSlash && srcPath.join("/").substr(-1) !== "/") {
      srcPath.push("");
    }

    const isAbsolute =
      srcPath[0] === "" || (srcPath[0] && srcPath[0].charAt(0) === "/");

    // put the host back
    if (noLeadingSlashes) {
      result.hostname = result.host = isAbsolute
        ? ""
        : srcPath.length
        ? srcPath.shift()
        : "";
      // Occasionally the auth can get stuck only in host.
      // This especially happens in cases like
      // url.resolveObject('mailto:local1@domain1', 'local2@domain2')
      const authInHost =
        result.host && result.host.indexOf("@") > 0
          ? result.host.split("@")
          : false;
      if (authInHost) {
        result.auth = authInHost.shift();
        result.host = result.hostname = authInHost.shift();
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
      result.path =
        (result.pathname ? result.pathname : "") +
        (result.search ? result.search : "");
    }
    result.auth = relative.auth || result.auth;
    result.slashes = result.slashes || relative.slashes;
    result.href = result.format();
    return result;
  }
  resolve(relative: string | Url): string {
    return this.resolveObject(parse(relative, false, true)).format();
  }
}

export function parse(
  url: Url | string,
  parseQueryString = true,
  slashesDenoteHost = true
): Url {
  if (url instanceof Url) return url;

  const urlObject = new Url();
  urlObject.parse(url, parseQueryString, slashesDenoteHost);
  return urlObject;
}

export function resolve(source: Url | string, relative: string | Url): string {
  return parse(source, false, true).resolve(relative);
}

export function resolveObject(
  source: Url,
  relative: string | Url
): Url | string {
  if (!source) return relative;
  return parse(source, false, true).resolveObject(relative);
}

function getHostname(self: Url, rest: string, hostname: string): string {
  for (let i = 0; i < hostname.length; ++i) {
    const code = hostname.charCodeAt(i);
    const isValid =
      (code >= CHAR_LOWERCASE_A && code <= CHAR_LOWERCASE_Z) ||
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

const noEscapeAuth = [
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0, // 0x00 - 0x0F
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0, // 0x10 - 0x1F
  0,
  1,
  0,
  0,
  0,
  0,
  0,
  1,
  1,
  1,
  1,
  0,
  0,
  1,
  1,
  0, // 0x20 - 0x2F
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  0,
  0,
  0,
  0,
  0, // 0x30 - 0x3F
  0,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1, // 0x40 - 0x4F
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  0,
  0,
  0,
  0,
  1, // 0x50 - 0x5F
  0,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1, // 0x60 - 0x6F
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  1,
  0,
  0,
  0,
  1,
  0, // 0x70 - 0x7F
];

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
  "%7D",
];

function autoEscapeStr(rest: string): string {
  let escaped = "";
  let lastEscapedPos = 0;
  for (let i = 0; i < rest.length; ++i) {
    // `escaped` contains substring up to the last escaped character.
    const escapedChar = escapedCodes[rest.charCodeAt(i)];
    if (escapedChar) {
      // Concat if there are ordinary characters in the middle.
      if (i > lastEscapedPos) escaped += rest.slice(lastEscapedPos, i);
      escaped += escapedChar;
      lastEscapedPos = i + 1;
    }
  }
  if (lastEscapedPos === 0)
    // Nothing has been escaped.
    return rest;

  // There are ordinary characters at the end.
  if (lastEscapedPos < rest.length) escaped += rest.slice(lastEscapedPos);

  return escaped;
}

export function fileURLToPath(path: string | URL | Url): string {
  if (typeof path === "string") path = new URL(path);
  else if (!(path instanceof URL))
    throw new Deno.errors.InvalidData(
      "invalid argument path , must be a string or URL"
    );
  if (path.protocol !== "file:")
    throw new Deno.errors.InvalidData("invalid url scheme");
  return isWindows ? getPathFromURLWin(path) : getPathFromURLPosix(path);
}

function getPathFromURLWin(url: URL): string {
  const hostname = url.hostname;
  let pathname = url.pathname;
  for (let n = 0; n < pathname.length; n++) {
    if (pathname[n] === "%") {
      const third = pathname.codePointAt(n + 2) || 0x20;
      if (
        (pathname[n + 1] === "2" && third === 102) || // 2f 2F /
        (pathname[n + 1] === "5" && third === 99)
      ) {
        // 5c 5C \
        throw new Deno.errors.InvalidData(
          "must not include encoded \\ or / characters"
        );
      }
    }
  }

  pathname = pathname.replace(forwardSlashRegEx, "\\");
  pathname = decodeURIComponent(pathname);
  if (hostname !== "") {
    //TODO add support for punycode encodings
    return `\\\\${hostname}${pathname}`;
  } else {
    // Otherwise, it's a local path that requires a drive letter
    const letter = pathname.codePointAt(1) || 0x20;
    const sep = pathname[2];
    if (
      letter < CHAR_LOWERCASE_A ||
      letter > CHAR_LOWERCASE_Z || // a..z A..Z
      sep !== ":"
    ) {
      throw new Deno.errors.InvalidData("file url path must be absolute");
    }
    return pathname.slice(1);
  }
}

function getPathFromURLPosix(url: URL): string {
  if (url.hostname !== "") {
    throw new Deno.errors.InvalidData("invalid file url hostname");
  }
  const pathname = url.pathname;
  for (let n = 0; n < pathname.length; n++) {
    if (pathname[n] === "%") {
      const third = pathname.codePointAt(n + 2) || 0x20;
      if (pathname[n + 1] === "2" && third === 102) {
        throw new Deno.errors.InvalidData(
          "must not include encoded / characters"
        );
      }
    }
  }
  return decodeURIComponent(pathname);
}

export function pathToFileURL(filepath: string): URL {
  let resolved = path.resolve(filepath);
  // path.resolve strips trailing slashes so we must add them back
  const filePathLast = filepath.charCodeAt(filepath.length - 1);
  if (
    (filePathLast === CHAR_FORWARD_SLASH ||
      (isWindows && filePathLast === CHAR_BACKWARD_SLASH)) &&
    resolved[resolved.length - 1] !== path.sep
  )
    resolved += "/";
  const outURL = new URL("file://");
  if (resolved.includes("%")) resolved = resolved.replace(percentRegEx, "%25");
  // In posix, "/" is a valid character in paths
  if (!isWindows && resolved.includes("\\"))
    resolved = resolved.replace(backslashRegEx, "%5C");
  if (resolved.includes("\n")) resolved = resolved.replace(newlineRegEx, "%0A");
  if (resolved.includes("\r"))
    resolved = resolved.replace(carriageReturnRegEx, "%0D");
  if (resolved.includes("\t")) resolved = resolved.replace(tabRegEx, "%09");
  outURL.pathname = resolved;
  return outURL;
}

type FormatOptions = {
  auth: boolean;
  fragment: boolean;
  search: boolean;
  unicode: boolean;
};

export function format(
  urlObject: Url | string,
  options?: FormatOptions
): string {
  // Ensure it's an object, and not a string url.
  // If it's an object, this is a no-op.
  // this way, you can call urlParse() on strings
  // to clean up potentially wonky urls.
  if (typeof urlObject === "string") {
    urlObject = parse(urlObject);
  } else if (typeof urlObject !== "object" || urlObject === null) {
    throw new Deno.errors.InvalidData(
      "invalid argument path , must be a string or object"
    );
  } else if (!(urlObject instanceof Url)) {
    const format: Url = urlObject[formatSymbol];
    return format
      ? format.call(urlObject, options)
      : Url.prototype.format.call(urlObject);
  }
  return urlObject.format();
}
