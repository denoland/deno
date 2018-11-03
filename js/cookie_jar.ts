/**
 * @module deno
 * @private
 */
import { notImplemented } from "./util";
// import * as cookie from 'cookie';

const cookieAttributeNames = [
  "Max-Age",
  "Expires",
  "HttpOnly",
  "Secure",
  "Path",
  "SameSite",
  "Domain"
];

/**
 * A jar for storing delicious cookies.
 * @class
 * @param {Response|Request} [parent] Underlying resource that contains cookies in headers
 */
class CookieJar {
  private cookies: any;
  private parent: any;

  constructor(parent) {
    this.parent = parent;
    if (parent instanceof Request)
      this.cookies = parseCookies(parent.headers.get("Cookie"));
    else if (parent instanceof Response)
      this.cookies = parseCookies(parent.headers.get("Set-Cookie"));
  }

  /**
   * Gets a cookie by name
   * @param {String} name
   */
  get(name) {
    return this.cookies.find(c => c.name === name);
  }

  /**
   * Sets a cookie, and applies it to the underlying {@linkcode Request} or {@linkcode Response}
   * @param {String} name
   * @param {String} value
   * @param {Object} [options]
   */
  append(name, value, options) {
    notImplemented();
    // const cookieStr = cookie.serialize(name, value, options)
    // this.cookies = this.cookies.concat(parseCookie(cookieStr))
    // if (this.parent instanceof Request)
    // 	this.parent.headers.append("Cookie", cookieStr)
    // else if (this.parent instanceof Response)
    // 	this.parent.headers.append("Set-Cookie", cookieStr)
  }
}

function parseCookies(rawCookies) {
  let cookies = [];
  for (let c of rawCookies) {
    cookies = cookies.concat(parseCookie(c));
  }
  return cookies;
}

function parseCookie(cookieStr) {
  notImplemented();
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
}

export { CookieJar };
export default CookieJar;
