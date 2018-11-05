/**
 * @module deno
 * @private
 */
import { notImplemented } from "./util";
import * as cookie from "cookie";
import * as domTypes from "./dom_types";

type Cookie = {
  name: string;
  value: string;
};

/**
 * A jar for storing delicious cookies.
 * @class
 * @param {Response|Request} [parent] Underlying resource that contains cookies in headers
 */
export class CookieJar {
  private cookies: Cookie[];
  private parent: domTypes.Request | domTypes.Response;

  constructor(parent: domTypes.Request | domTypes.Response) {
    this.parent = parent;
    if (isRequest(parent))
      this.cookies = parseCookies([parent.headers.get("Cookie") as string]);
    else if (isResponse(parent))
      this.cookies = parseCookies([parent.headers.get("Set-Cookie") as string]);
  }

  get(name: string) {
    return this.cookies.find((c: Cookie) => c.name === name);
  }

  /** Sets a cookie */
  append(name: string, value: string, options: cookie.CookieSerializeOptions) {
    notImplemented();
    const cookieStr = cookie.serialize(name, value, options);
    this.cookies = this.cookies.concat(parseCookie(cookieStr));
    if (isRequest(this.parent)) this.parent.headers.append("Cookie", cookieStr);
    else if (isResponse(this.parent))
      this.parent.headers.append("Set-Cookie", cookieStr);
  }
}

function parseCookies(rawCookies: string[] | Cookie[]): Cookie[] {
  let cookies: Cookie[] = [];
  for (const c of rawCookies) {
    cookies = cookies.concat(parseCookie(c));
  }
  return cookies;
}

function parseCookie(cookieStr: string | Cookie): Cookie {
  notImplemented();
  // const cookieAttributeNames = [
  //   "Max-Age",
  //   "Expires",
  //   "HttpOnly",
  //   "Secure",
  //   "Path",
  //   "SameSite",
  //   "Domain"
  // ];
  // let options = {}
  // let cookies = []
  // let parsed = cookie.parse(cookieStr)
  // for (let k in parsed) {
  // 	if (cookieAttributeNames.indexOf(k) != -1) {
  // 		options[k] = parsed[k]
  // 		continue
  // 	}
  // 	cookies.push({ name: k, value: parsed[k] })
  // }
  // return cookies.map((c) => Object.assign(c, options))
  return {} as Cookie;
}

function isResponse(object: any): object is domTypes.Response {
  return "ok" in object;
}

function isRequest(object: any): object is domTypes.Request {
  return "cache" in object;
}
