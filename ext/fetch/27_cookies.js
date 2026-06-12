// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const {
  op_cookie_jar_clear,
  op_cookie_jar_cookie_header,
  op_cookie_jar_delete_cookie,
  op_cookie_jar_entries,
  op_cookie_jar_get_cookies,
  op_cookie_jar_new,
  op_cookie_jar_set_cookie,
  op_cookie_parse,
  op_cookie_parse_cookie_header,
  op_cookie_serialize,
} = core.ops;

const { internalRidSymbol } = core;
const {
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  DatePrototype,
  DatePrototypeGetTime,
  FunctionPrototypeCall,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeGetSize,
  MapPrototypeHas,
  MapPrototypeSet,
  Number,
  NumberIsFinite,
  ObjectDefineProperty,
  ObjectFreeze,
  ObjectPrototypeIsPrototypeOf,
  SafeArrayIterator,
  SafeMap,
  SafeMapIterator,
  SymbolDispose,
  SymbolIterator,
  TypeError,
} = primordials;

/**
 * Normalizes a user provided cookie (string in `Set-Cookie` format or a
 * `Deno.Cookie`-shaped object) into the plain object the ops expect.
 */
function normalizeCookie(cookie, prefix) {
  if (typeof cookie === "string") {
    return op_cookie_parse(cookie);
  }
  if (cookie === null || typeof cookie !== "object") {
    throw new TypeError(
      `${prefix}: 'cookie' must be a string or an object`,
    );
  }
  let expires = cookie.expires;
  if (expires != null) {
    if (ObjectPrototypeIsPrototypeOf(DatePrototype, expires)) {
      expires = DatePrototypeGetTime(expires);
    } else {
      expires = Number(expires);
    }
    if (!NumberIsFinite(expires)) {
      throw new TypeError(`${prefix}: 'expires' must be a finite time value`);
    }
  }
  let maxAge = cookie.maxAge;
  if (maxAge != null) {
    maxAge = Number(maxAge);
    if (!NumberIsFinite(maxAge)) {
      throw new TypeError(`${prefix}: 'maxAge' must be a finite number`);
    }
  }
  return {
    __proto__: null,
    name: `${cookie.name ?? ""}`,
    value: `${cookie.value ?? ""}`,
    domain: cookie.domain != null ? `${cookie.domain}` : null,
    path: cookie.path != null ? `${cookie.path}` : null,
    expires: expires ?? null,
    maxAge: maxAge ?? null,
    secure: !!cookie.secure,
    httpOnly: !!cookie.httpOnly,
    sameSite: cookie.sameSite != null ? `${cookie.sameSite}` : null,
    partitioned: !!cookie.partitioned,
  };
}

function urlString(url) {
  if (url === undefined) {
    return "";
  }
  return `${url}`;
}

/**
 * A cookie store implementing the RFC 6265bis storage model. Attach it to an
 * HTTP client via `Deno.createHttpClient({ cookieJar })` to have cookies
 * persisted and sent automatically across `fetch` calls.
 */
class CookieJar {
  #rid;

  /**
   * @param {Deno.Cookie[]=} cookies initial cookies; each must carry an
   *   explicit domain.
   */
  constructor(cookies = undefined) {
    const rid = op_cookie_jar_new();
    ObjectDefineProperty(this, internalRidSymbol, {
      __proto__: null,
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
    if (cookies !== undefined) {
      try {
        for (const cookie of new SafeArrayIterator(cookies)) {
          this.setCookie(cookie);
        }
      } catch (e) {
        core.close(rid);
        throw e;
      }
    }
  }

  /**
   * Returns the cookies that would be sent in a request to `url`, or all
   * cookies in the jar when no URL is given.
   * @returns {Deno.Cookie[]}
   */
  getCookies(url = undefined) {
    if (url === undefined) {
      return op_cookie_jar_entries(this.#rid);
    }
    return op_cookie_jar_get_cookies(this.#rid, `${url}`);
  }

  /**
   * Returns the `Cookie` header value for a request to `url`, or `null`
   * when no cookies match.
   * @returns {string | null}
   */
  getCookieString(url) {
    const header = op_cookie_jar_cookie_header(this.#rid, `${url}`);
    return header === "" ? null : header;
  }

  /**
   * Stores a cookie. `cookie` is a `Deno.Cookie`-shaped object or a string
   * in `Set-Cookie` format. When `url` is provided the cookie is stored as
   * if it was received in a response from that URL (with all the matching
   * rules that implies); otherwise the cookie must carry an explicit domain.
   */
  setCookie(cookie, url = undefined) {
    op_cookie_jar_set_cookie(
      this.#rid,
      normalizeCookie(cookie, "Failed to execute 'CookieJar.setCookie'"),
      urlString(url),
    );
  }

  /**
   * Deletes cookies with the given name, optionally narrowed by domain and
   * path. Returns the number of cookies that were removed.
   * @returns {number}
   */
  deleteCookie(name, options = undefined) {
    return op_cookie_jar_delete_cookie(
      this.#rid,
      `${name}`,
      options?.domain != null ? `${options.domain}` : "",
      options?.path != null ? `${options.path}` : "",
    );
  }

  /** Removes all cookies from the jar. */
  clear() {
    op_cookie_jar_clear(this.#rid);
  }

  /** @returns {Deno.Cookie[]} */
  toJSON() {
    return op_cookie_jar_entries(this.#rid);
  }

  close() {
    core.close(this.#rid);
  }

  [SymbolDispose]() {
    core.tryClose(this.#rid);
  }
}
const CookieJarPrototype = CookieJar.prototype;

/**
 * Utilities for parsing and serializing individual cookies in `Set-Cookie`
 * format.
 */
const Cookie = ObjectFreeze({
  __proto__: null,
  /**
   * Parses a `Set-Cookie` header value into a `Deno.Cookie`. Throws when
   * the header is not a valid cookie.
   * @returns {Deno.Cookie}
   */
  parse(setCookie) {
    return op_cookie_parse(`${setCookie}`);
  },
  /**
   * Serializes a `Deno.Cookie` into a `Set-Cookie` header value, validating
   * the name, value and attributes.
   * @returns {string}
   */
  serialize(cookie) {
    return op_cookie_serialize(
      normalizeCookie(cookie, "Failed to execute 'Cookie.serialize'"),
    );
  },
});

/**
 * A Map-like view over the cookies of a `Cookie` request header, for use in
 * servers (`Deno.serve()`, `node:http`):
 *
 * ```js
 * const cookies = new Deno.CookieMap(request.headers);
 * cookies.get("session");
 * cookies.set("theme", "dark", { path: "/" });
 * for (const header of cookies.toSetCookieStrings()) {
 *   response.headers.append("set-cookie", header);
 * }
 * ```
 */
class CookieMap {
  /** @type {SafeMap<string, string>} */
  #map = new SafeMap();
  /** Serialized Set-Cookie strings for mutations made through this map. */
  #changes = new SafeMap();

  /**
   * @param init a `Cookie` header string, or a `Headers` object (the
   *   `cookie` header is read), or null/undefined for an empty map.
   */
  constructor(init = undefined) {
    if (init == null) {
      return;
    }
    let header;
    if (typeof init === "string") {
      header = init;
    } else if (typeof init === "object" && typeof init.get === "function") {
      header = init.get("cookie") ?? "";
    } else {
      throw new TypeError(
        "Failed to construct 'CookieMap': 'init' must be a string or a Headers object",
      );
    }
    const pairs = op_cookie_parse_cookie_header(header);
    for (const pair of new SafeArrayIterator(pairs)) {
      // The first occurrence of a duplicate name wins.
      if (!MapPrototypeHas(this.#map, pair[0])) {
        MapPrototypeSet(this.#map, pair[0], pair[1]);
      }
    }
  }

  /** @returns {number} */
  get size() {
    return MapPrototypeGetSize(this.#map);
  }

  /** @returns {string | undefined} */
  get(name) {
    return MapPrototypeGet(this.#map, `${name}`);
  }

  /** @returns {boolean} */
  has(name) {
    return MapPrototypeHas(this.#map, `${name}`);
  }

  /**
   * Sets a cookie value. Attributes for the generated `Set-Cookie` header
   * (domain, path, expires, maxAge, secure, httpOnly, sameSite,
   * partitioned) may be passed via `options`. The name, value and
   * attributes are validated eagerly.
   */
  set(name, value, options = undefined) {
    name = `${name}`;
    value = `${value}`;
    const cookie = normalizeCookie(
      { __proto__: null, ...options, name, value },
      "Failed to execute 'CookieMap.set'",
    );
    const setCookie = op_cookie_serialize(cookie);
    MapPrototypeSet(this.#map, name, value);
    MapPrototypeSet(this.#changes, name, setCookie);
    return this;
  }

  /**
   * Deletes a cookie. The generated `Set-Cookie` header expires the cookie
   * immediately; pass `domain`/`path` via `options` if the cookie was set
   * with them.
   * @returns {boolean} whether the cookie was in the map.
   */
  delete(name, options = undefined) {
    name = `${name}`;
    const cookie = normalizeCookie(
      {
        __proto__: null,
        ...options,
        name,
        value: "",
        expires: 0,
        maxAge: 0,
      },
      "Failed to execute 'CookieMap.delete'",
    );
    const setCookie = op_cookie_serialize(cookie);
    MapPrototypeSet(this.#changes, name, setCookie);
    return MapPrototypeDelete(this.#map, name);
  }

  /** @returns {[string, string][]} */
  entries() {
    const out = [];
    for (const entry of new SafeMapIterator(this.#map)) {
      ArrayPrototypePush(out, [entry[0], entry[1]]);
    }
    return out;
  }

  /** @returns {string[]} */
  keys() {
    const out = [];
    for (const entry of new SafeMapIterator(this.#map)) {
      ArrayPrototypePush(out, entry[0]);
    }
    return out;
  }

  /** @returns {string[]} */
  values() {
    const out = [];
    for (const entry of new SafeMapIterator(this.#map)) {
      ArrayPrototypePush(out, entry[1]);
    }
    return out;
  }

  forEach(callback, thisArg = undefined) {
    for (const entry of new SafeMapIterator(this.#map)) {
      FunctionPrototypeCall(callback, thisArg, entry[1], entry[0], this);
    }
  }

  [SymbolIterator]() {
    return new SafeMapIterator(this.#map);
  }

  /**
   * Serializes the current cookies into a `Cookie` request header value.
   * @returns {string}
   */
  toString() {
    const out = [];
    for (const entry of new SafeMapIterator(this.#map)) {
      ArrayPrototypePush(out, `${entry[0]}=${entry[1]}`);
    }
    return ArrayPrototypeJoin(out, "; ");
  }

  /** @returns {Record<string, string>} */
  toJSON() {
    const out = { __proto__: null };
    for (const entry of new SafeMapIterator(this.#map)) {
      out[entry[0]] = entry[1];
    }
    return out;
  }

  /**
   * Returns one `Set-Cookie` header value for every mutation (set or
   * delete) made through this map, to be applied to a response.
   * @returns {string[]}
   */
  toSetCookieStrings() {
    const out = [];
    for (const entry of new SafeMapIterator(this.#changes)) {
      ArrayPrototypePush(out, entry[1]);
    }
    return out;
  }
}
const CookieMapPrototype = CookieMap.prototype;

return {
  Cookie,
  CookieJar,
  CookieJarPrototype,
  CookieMap,
  CookieMapPrototype,
};
})();
