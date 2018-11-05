/**
 * @module fetch
 */
import { CookieJar } from "./cookie_jar";
import { DenoBody } from "./body";
import * as domTypes from "./dom_types";
import { DenoHeaders } from "./headers";
import { notImplemented } from "./util";

function byteUpperCase(s: string) {
  return String(s).replace(/[a-z]/g, function byteUpperCaseReplace(c) {
    return c.toUpperCase();
  });
}

function normalizeMethod(m: string) {
  var u = byteUpperCase(m);
  if (
    u === "DELETE" ||
    u === "GET" ||
    u === "HEAD" ||
    u === "OPTIONS" ||
    u === "POST" ||
    u === "PUT"
  )
    return u;
  return m;
}

interface DenoRequestInit extends domTypes.RequestInit {
  remoteAddr?: string;
}

/**
 * An HTTP request
 * @param {Blob|String} [body]
 * @param {Object} [init]
 * @mixes Body
 */
export class DenoRequest extends DenoBody implements domTypes.Request {
  method: string;
  url: string;
  referrer: string;
  mode: domTypes.RequestMode;
  credentials: domTypes.RequestCredentials;
  headers: DenoHeaders;
  remoteAddr: string;
  cache: domTypes.RequestCache;
  destination: domTypes.RequestDestination;
  integrity: string;
  isHistoryNavigation: boolean;
  isReloadNavigation: boolean;
  keepalive: boolean;
  redirect: domTypes.RequestRedirect;
  referrerPolicy: domTypes.ReferrerPolicy;
  signal: domTypes.AbortSignal;

  private cookieJar: CookieJar;

  constructor(input: string | domTypes.Request, init?: DenoRequestInit) {
    if (arguments.length < 1) throw TypeError("Not enough arguments");

    let body = null;
    if (init && init.body) {
      body = init.body;
    }
    if (!body && input instanceof DenoRequest) {
      if (input.bodyUsed) throw TypeError();
      // grab request body if we can
      body = input.bodySource;
    }
    // logger.debug('creating request! body typeof:', typeof Body, typeof init.body)
    super(body as domTypes.BodyInit);

    // readonly attribute ByteString method;
    /**
     * The HTTP request method
     * @readonly
     * @default GET
     * @type {string}
     */
    this.method = "GET";

    // readonly attribute USVString url;
    /**
     * The request URL
     * @readonly
     * @type {string}
     */
    this.url = "";

    // readonly attribute DOMString referrer;
    this.referrer = ""; // TODO: Implement.

    // readonly attribute RequestMode mode;
    this.mode = "same-origin"; // TODO: Implement.

    // readonly attribute RequestCredentials credentials;
    this.credentials = "omit";

    if (input instanceof DenoRequest) {
      if (input.bodyUsed) throw TypeError();
      this.method = input.method;
      this.url = input.url;
      this.headers = new DenoHeaders(input.headers);
      this.credentials = input.credentials;
      this.stream = input.stream;
      this.remoteAddr = input.remoteAddr;
      this.referrer = input.referrer;
      this.mode = input.mode;
    } else {
      this.headers = new DenoHeaders({});
      this.url = <string>input;
    }

    init = Object(init);

    if (init && init.remoteAddr) {
      this.remoteAddr = init.remoteAddr;
    }

    if (init && init.method) {
      this.method = normalizeMethod(init.method);
    }

    if (init && "headers" in init) {
      /**
       * Headers sent with the request.
       * @type {Headers}
       */
      this.headers = new DenoHeaders(init.headers);
    }

    if (
      init &&
      init.credentials &&
      ["omit", "same-origin", "include"].indexOf(init.credentials) !== -1
    )
      this.credentials = init.credentials;
  }

  get cookies() {
    if (this.cookieJar) return this.cookieJar;
    this.cookieJar = new CookieJar(this);
    return this.cookieJar;
  }

  clone() {
    notImplemented();
    return {} as DenoRequest;
    // if (this.bodyUsed)
    // 	throw new Error("body has already been used")
    // let body2 = this.bodySource

    // // if (this.bodySource instanceof DenoBody) {
    // // 	const tees = this.body.tee()
    // // 	this.stream = this.bodySource = tees[0]
    // // 	body2 = tees[1]
    // // }
    // const cloned = new DenoRequest(this.url, {
    // 	body: body2,
    // 	remoteAddr: this.remoteAddr,
    // 	method: this.method,
    // 	headers: this.headers,
    // 	credentials: this.credentials
    // })
    // return cloned
  }
}
