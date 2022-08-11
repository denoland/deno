// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const webidl = window.__bootstrap.webidl;
  const {
    ArrayIsArray,
    ArrayPrototypeMap,
    ArrayPrototypePush,
    ArrayPrototypeSome,
    ArrayPrototypeSort,
    ArrayPrototypeSplice,
    ObjectKeys,
    SafeArrayIterator,
    StringPrototypeSlice,
    StringPrototypeSplit,
    Symbol,
    SymbolFor,
    SymbolIterator,
    TypeError,
  } = window.__bootstrap.primordials;

  const _list = Symbol("list");
  const _urlObject = Symbol("url object");

  // WARNING: must match rust code's UrlSetter::*
  const SET_HASH = 0;
  const SET_HOST = 1;
  const SET_HOSTNAME = 2;
  const SET_PASSWORD = 3;
  const SET_PATHNAME = 4;
  const SET_PORT = 5;
  const SET_PROTOCOL = 6;
  const SET_SEARCH = 7;
  const SET_USERNAME = 8;

  // Helper functions
  function opUrlReparse(href, setter, value) {
    return _urlParts(
      ops.op_url_reparse(href, [setter, value]),
    );
  }
  function opUrlParse(href, maybeBase) {
    return _urlParts(ops.op_url_parse(href, maybeBase));
  }
  function _urlParts(internalParts) {
    // WARNING: must match UrlParts serialization rust's url_result()
    const {
      0: href,
      1: hash,
      2: host,
      3: hostname,
      4: origin,
      5: password,
      6: pathname,
      7: port,
      8: protocol,
      9: search,
      10: username,
    } = StringPrototypeSplit(internalParts, "\n");
    return {
      href,
      hash,
      host,
      hostname,
      origin,
      password,
      pathname,
      port,
      protocol,
      search,
      username,
    };
  }

  class URLSearchParams {
    [_list];
    [_urlObject] = null;

    /**
     * @param {string | [string][] | Record<string, string>} init
     */
    constructor(init = "") {
      const prefix = "Failed to construct 'URL'";
      init = webidl.converters
        ["sequence<sequence<USVString>> or record<USVString, USVString> or USVString"](
          init,
          { prefix, context: "Argument 1" },
        );
      this[webidl.brand] = webidl.brand;

      if (typeof init === "string") {
        // Overload: USVString
        // If init is a string and starts with U+003F (?),
        // remove the first code point from init.
        if (init[0] == "?") {
          init = StringPrototypeSlice(init, 1);
        }
        this[_list] = ops.op_url_parse_search_params(init);
      } else if (ArrayIsArray(init)) {
        // Overload: sequence<sequence<USVString>>
        this[_list] = ArrayPrototypeMap(init, (pair, i) => {
          if (pair.length !== 2) {
            throw new TypeError(
              `${prefix}: Item ${
                i + 0
              } in the parameter list does have length 2 exactly.`,
            );
          }
          return [pair[0], pair[1]];
        });
      } else {
        // Overload: record<USVString, USVString>
        this[_list] = ArrayPrototypeMap(
          ObjectKeys(init),
          (key) => [key, init[key]],
        );
      }
    }

    #updateUrlSearch() {
      const url = this[_urlObject];
      if (url === null) {
        return;
      }
      url[_url] = opUrlReparse(url.href, SET_SEARCH, this.toString());
    }

    /**
     * @param {string} name
     * @param {string} value
     */
    append(name, value) {
      webidl.assertBranded(this, URLSearchParamsPrototype);
      const prefix = "Failed to execute 'append' on 'URLSearchParams'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      name = webidl.converters.USVString(name, {
        prefix,
        context: "Argument 1",
      });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 2",
      });
      ArrayPrototypePush(this[_list], [name, value]);
      this.#updateUrlSearch();
    }

    /**
     * @param {string} name
     */
    delete(name) {
      webidl.assertBranded(this, URLSearchParamsPrototype);
      const prefix = "Failed to execute 'append' on 'URLSearchParams'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.USVString(name, {
        prefix,
        context: "Argument 1",
      });
      const list = this[_list];
      let i = 0;
      while (i < list.length) {
        if (list[i][0] === name) {
          ArrayPrototypeSplice(list, i, 1);
        } else {
          i++;
        }
      }
      this.#updateUrlSearch();
    }

    /**
     * @param {string} name
     * @returns {string[]}
     */
    getAll(name) {
      webidl.assertBranded(this, URLSearchParamsPrototype);
      const prefix = "Failed to execute 'getAll' on 'URLSearchParams'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.USVString(name, {
        prefix,
        context: "Argument 1",
      });
      const values = [];
      for (const entry of this[_list]) {
        if (entry[0] === name) {
          ArrayPrototypePush(values, entry[1]);
        }
      }
      return values;
    }

    /**
     * @param {string} name
     * @return {string | null}
     */
    get(name) {
      webidl.assertBranded(this, URLSearchParamsPrototype);
      const prefix = "Failed to execute 'get' on 'URLSearchParams'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.USVString(name, {
        prefix,
        context: "Argument 1",
      });
      for (const entry of this[_list]) {
        if (entry[0] === name) {
          return entry[1];
        }
      }
      return null;
    }

    /**
     * @param {string} name
     * @return {boolean}
     */
    has(name) {
      webidl.assertBranded(this, URLSearchParamsPrototype);
      const prefix = "Failed to execute 'has' on 'URLSearchParams'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.USVString(name, {
        prefix,
        context: "Argument 1",
      });
      return ArrayPrototypeSome(this[_list], (entry) => entry[0] === name);
    }

    /**
     * @param {string} name
     * @param {string} value
     */
    set(name, value) {
      webidl.assertBranded(this, URLSearchParamsPrototype);
      const prefix = "Failed to execute 'set' on 'URLSearchParams'";
      webidl.requiredArguments(arguments.length, 2, { prefix });
      name = webidl.converters.USVString(name, {
        prefix,
        context: "Argument 1",
      });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 2",
      });

      const list = this[_list];

      // If there are any name-value pairs whose name is name, in list,
      // set the value of the first such name-value pair to value
      // and remove the others.
      let found = false;
      let i = 0;
      while (i < list.length) {
        if (list[i][0] === name) {
          if (!found) {
            list[i][1] = value;
            found = true;
            i++;
          } else {
            ArrayPrototypeSplice(list, i, 1);
          }
        } else {
          i++;
        }
      }

      // Otherwise, append a new name-value pair whose name is name
      // and value is value, to list.
      if (!found) {
        ArrayPrototypePush(list, [name, value]);
      }

      this.#updateUrlSearch();
    }

    sort() {
      webidl.assertBranded(this, URLSearchParamsPrototype);
      ArrayPrototypeSort(
        this[_list],
        (a, b) => (a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1),
      );
      this.#updateUrlSearch();
    }

    /**
     * @return {string}
     */
    toString() {
      webidl.assertBranded(this, URLSearchParamsPrototype);
      return ops.op_url_stringify_search_params(this[_list]);
    }
  }

  webidl.mixinPairIterable("URLSearchParams", URLSearchParams, _list, 0, 1);

  webidl.configurePrototype(URLSearchParams);
  const URLSearchParamsPrototype = URLSearchParams.prototype;

  webidl.converters["URLSearchParams"] = webidl.createInterfaceConverter(
    "URLSearchParams",
    URLSearchParamsPrototype,
  );

  const _url = Symbol("url");

  class URL {
    [_url];
    #queryObject = null;

    /**
     * @param {string} url
     * @param {string} base
     */
    constructor(url, base = undefined) {
      const prefix = "Failed to construct 'URL'";
      url = webidl.converters.DOMString(url, { prefix, context: "Argument 1" });
      if (base !== undefined) {
        base = webidl.converters.DOMString(base, {
          prefix,
          context: "Argument 2",
        });
      }
      this[webidl.brand] = webidl.brand;
      this[_url] = opUrlParse(url, base);
    }

    [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
      const object = {
        href: this.href,
        origin: this.origin,
        protocol: this.protocol,
        username: this.username,
        password: this.password,
        host: this.host,
        hostname: this.hostname,
        port: this.port,
        pathname: this.pathname,
        hash: this.hash,
        search: this.search,
      };
      return `${this.constructor.name} ${inspect(object, inspectOptions)}`;
    }

    #updateSearchParams() {
      if (this.#queryObject !== null) {
        const params = this.#queryObject[_list];
        const newParams = ops.op_url_parse_search_params(
          StringPrototypeSlice(this.search, 1),
        );
        ArrayPrototypeSplice(
          params,
          0,
          params.length,
          ...new SafeArrayIterator(newParams),
        );
      }
    }

    /** @return {string} */
    get hash() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].hash;
    }

    /** @param {string} value */
    set hash(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'hash' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = opUrlReparse(this[_url].href, SET_HASH, value);
      } catch {
        /* pass */
      }
    }

    /** @return {string} */
    get host() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].host;
    }

    /** @param {string} value */
    set host(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'host' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = opUrlReparse(this[_url].href, SET_HOST, value);
      } catch {
        /* pass */
      }
    }

    /** @return {string} */
    get hostname() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].hostname;
    }

    /** @param {string} value */
    set hostname(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'hostname' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = opUrlReparse(this[_url].href, SET_HOSTNAME, value);
      } catch {
        /* pass */
      }
    }

    /** @return {string} */
    get href() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].href;
    }

    /** @param {string} value */
    set href(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'href' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      this[_url] = opUrlParse(value);
      this.#updateSearchParams();
    }

    /** @return {string} */
    get origin() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].origin;
    }

    /** @return {string} */
    get password() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].password;
    }

    /** @param {string} value */
    set password(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'password' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = opUrlReparse(this[_url].href, SET_PASSWORD, value);
      } catch {
        /* pass */
      }
    }

    /** @return {string} */
    get pathname() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].pathname;
    }

    /** @param {string} value */
    set pathname(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'pathname' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = opUrlReparse(this[_url].href, SET_PATHNAME, value);
      } catch {
        /* pass */
      }
    }

    /** @return {string} */
    get port() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].port;
    }

    /** @param {string} value */
    set port(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'port' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = opUrlReparse(this[_url].href, SET_PORT, value);
      } catch {
        /* pass */
      }
    }

    /** @return {string} */
    get protocol() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].protocol;
    }

    /** @param {string} value */
    set protocol(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'protocol' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = opUrlReparse(this[_url].href, SET_PROTOCOL, value);
      } catch {
        /* pass */
      }
    }

    /** @return {string} */
    get search() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].search;
    }

    /** @param {string} value */
    set search(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'search' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = opUrlReparse(this[_url].href, SET_SEARCH, value);
        this.#updateSearchParams();
      } catch {
        /* pass */
      }
    }

    /** @return {string} */
    get username() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].username;
    }

    /** @param {string} value */
    set username(value) {
      webidl.assertBranded(this, URLPrototype);
      const prefix = "Failed to set 'username' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.DOMString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = opUrlReparse(this[_url].href, SET_USERNAME, value);
      } catch {
        /* pass */
      }
    }

    /** @return {string} */
    get searchParams() {
      if (this.#queryObject == null) {
        this.#queryObject = new URLSearchParams(this.search);
        this.#queryObject[_urlObject] = this;
      }
      return this.#queryObject;
    }

    /** @return {string} */
    toString() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].href;
    }

    /** @return {string} */
    toJSON() {
      webidl.assertBranded(this, URLPrototype);
      return this[_url].href;
    }
  }

  webidl.configurePrototype(URL);
  const URLPrototype = URL.prototype;

  /**
   * This function implements application/x-www-form-urlencoded parsing.
   * https://url.spec.whatwg.org/#concept-urlencoded-parser
   * @param {Uint8Array} bytes
   * @returns {[string, string][]}
   */
  function parseUrlEncoded(bytes) {
    return ops.op_url_parse_search_params(null, bytes);
  }

  webidl
    .converters[
      "sequence<sequence<USVString>> or record<USVString, USVString> or USVString"
    ] = (V, opts) => {
      // Union for (sequence<sequence<USVString>> or record<USVString, USVString> or USVString)
      if (webidl.type(V) === "Object" && V !== null) {
        if (V[SymbolIterator] !== undefined) {
          return webidl.converters["sequence<sequence<USVString>>"](V, opts);
        }
        return webidl.converters["record<USVString, USVString>"](V, opts);
      }
      return webidl.converters.USVString(V, opts);
    };

  window.__bootstrap.url = {
    URL,
    URLPrototype,
    URLSearchParams,
    URLSearchParamsPrototype,
    parseUrlEncoded,
  };
})(this);
