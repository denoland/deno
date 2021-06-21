// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const webidl = window.__bootstrap.webidl;

  const _list = Symbol("list");
  const _urlObject = Symbol("url object");

  class URLSearchParams {
    [_list];
    [_urlObject] = null;

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
          init = init.slice(1);
        }
        this[_list] = core.opSync("op_url_parse_search_params", init);
      } else if (Array.isArray(init)) {
        // Overload: sequence<sequence<USVString>>
        this[_list] = init.map((pair, i) => {
          if (pair.length !== 2) {
            throw new TypeError(
              `${prefix}: Item ${i +
                0} in the parameter list does have length 2 exactly.`,
            );
          }
          return [pair[0], pair[1]];
        });
      } else {
        // Overload: record<USVString, USVString>
        this[_list] = Object.keys(init).map((key) => [key, init[key]]);
      }
    }

    #updateUrlSearch() {
      const url = this[_urlObject];
      if (url === null) {
        return;
      }
      const parts = core.opSync("op_url_parse", {
        href: url.href,
        setSearch: this.toString(),
      });
      url[_url] = parts;
    }

    append(name, value) {
      webidl.assertBranded(this, URLSearchParams);
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
      this[_list].push([name, value]);
      this.#updateUrlSearch();
    }

    delete(name) {
      webidl.assertBranded(this, URLSearchParams);
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
          list.splice(i, 1);
        } else {
          i++;
        }
      }
      this.#updateUrlSearch();
    }

    getAll(name) {
      webidl.assertBranded(this, URLSearchParams);
      const prefix = "Failed to execute 'getAll' on 'URLSearchParams'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.USVString(name, {
        prefix,
        context: "Argument 1",
      });
      const values = [];
      for (const entry of this[_list]) {
        if (entry[0] === name) {
          values.push(entry[1]);
        }
      }
      return values;
    }

    get(name) {
      webidl.assertBranded(this, URLSearchParams);
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

    has(name) {
      webidl.assertBranded(this, URLSearchParams);
      const prefix = "Failed to execute 'has' on 'URLSearchParams'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      name = webidl.converters.USVString(name, {
        prefix,
        context: "Argument 1",
      });
      return this[_list].some((entry) => entry[0] === name);
    }

    set(name, value) {
      webidl.assertBranded(this, URLSearchParams);
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
            list.splice(i, 1);
          }
        } else {
          i++;
        }
      }

      // Otherwise, append a new name-value pair whose name is name
      // and value is value, to list.
      if (!found) {
        list.push([name, value]);
      }

      this.#updateUrlSearch();
    }

    sort() {
      webidl.assertBranded(this, URLSearchParams);
      this[_list].sort((a, b) => (a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1));
      this.#updateUrlSearch();
    }

    toString() {
      webidl.assertBranded(this, URLSearchParams);
      return core.opSync("op_url_stringify_search_params", this[_list]);
    }

    get [Symbol.toStringTag]() {
      return "URLSearchParams";
    }
  }

  webidl.mixinPairIterable("URLSearchParams", URLSearchParams, _list, 0, 1);

  webidl.configurePrototype(URLSearchParams);

  const _url = Symbol("url");

  class URL {
    [_url];
    #queryObject = null;

    constructor(url, base = undefined) {
      const prefix = "Failed to construct 'URL'";
      url = webidl.converters.USVString(url, { prefix, context: "Argument 1" });
      if (base !== undefined) {
        base = webidl.converters.USVString(base, {
          prefix,
          context: "Argument 2",
        });
      }
      this[webidl.brand] = webidl.brand;

      const parts = core.opSync("op_url_parse", { href: url, baseHref: base });
      this[_url] = parts;
    }

    [Symbol.for("Deno.customInspect")](inspect) {
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
      return `${this.constructor.name} ${inspect(object)}`;
    }

    #updateSearchParams() {
      if (this.#queryObject !== null) {
        const params = this.#queryObject[_list];
        const newParams = core.opSync(
          "op_url_parse_search_params",
          this.search.slice(1),
        );
        params.splice(0, params.length, ...newParams);
      }
    }

    get hash() {
      webidl.assertBranded(this, URL);
      return this[_url].hash;
    }

    set hash(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'hash' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = core.opSync("op_url_parse", {
          href: this.href,
          setHash: value,
        });
      } catch {
        /* pass */
      }
    }

    get host() {
      webidl.assertBranded(this, URL);
      return this[_url].host;
    }

    set host(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'host' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = core.opSync("op_url_parse", {
          href: this.href,
          setHost: value,
        });
      } catch {
        /* pass */
      }
    }

    get hostname() {
      webidl.assertBranded(this, URL);
      return this[_url].hostname;
    }

    set hostname(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'hostname' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = core.opSync("op_url_parse", {
          href: this.href,
          setHostname: value,
        });
      } catch {
        /* pass */
      }
    }

    get href() {
      webidl.assertBranded(this, URL);
      return this[_url].href;
    }

    set href(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'href' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      this[_url] = core.opSync("op_url_parse", {
        href: value,
      });
      this.#updateSearchParams();
    }

    get origin() {
      webidl.assertBranded(this, URL);
      return this[_url].origin;
    }

    get password() {
      webidl.assertBranded(this, URL);
      return this[_url].password;
    }

    set password(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'password' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = core.opSync("op_url_parse", {
          href: this.href,
          setPassword: value,
        });
      } catch {
        /* pass */
      }
    }

    get pathname() {
      webidl.assertBranded(this, URL);
      return this[_url].pathname;
    }

    set pathname(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'pathname' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = core.opSync("op_url_parse", {
          href: this.href,
          setPathname: value,
        });
      } catch {
        /* pass */
      }
    }

    get port() {
      webidl.assertBranded(this, URL);
      return this[_url].port;
    }

    set port(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'port' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = core.opSync("op_url_parse", {
          href: this.href,
          setPort: value,
        });
      } catch {
        /* pass */
      }
    }

    get protocol() {
      webidl.assertBranded(this, URL);
      return this[_url].protocol;
    }

    set protocol(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'protocol' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = core.opSync("op_url_parse", {
          href: this.href,
          setProtocol: value,
        });
      } catch {
        /* pass */
      }
    }

    get search() {
      webidl.assertBranded(this, URL);
      return this[_url].search;
    }

    set search(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'search' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = core.opSync("op_url_parse", {
          href: this.href,
          setSearch: value,
        });
        this.#updateSearchParams();
      } catch {
        /* pass */
      }
    }

    get username() {
      webidl.assertBranded(this, URL);
      return this[_url].username;
    }

    set username(value) {
      webidl.assertBranded(this, URL);
      const prefix = "Failed to set 'username' on 'URL'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.USVString(value, {
        prefix,
        context: "Argument 1",
      });
      try {
        this[_url] = core.opSync("op_url_parse", {
          href: this.href,
          setUsername: value,
        });
      } catch {
        /* pass */
      }
    }

    get searchParams() {
      if (this.#queryObject == null) {
        this.#queryObject = new URLSearchParams(this.search);
        this.#queryObject[_urlObject] = this;
      }
      return this.#queryObject;
    }

    toString() {
      webidl.assertBranded(this, URL);
      return this.href;
    }

    toJSON() {
      webidl.assertBranded(this, URL);
      return this.href;
    }

    get [Symbol.toStringTag]() {
      return "URL";
    }
  }

  webidl.configurePrototype(URL);

  /**
   * This function implements application/x-www-form-urlencoded parsing.
   * https://url.spec.whatwg.org/#concept-urlencoded-parser
   * @param {Uint8Array} bytes
   * @returns {[string, string][]}
   */
  function parseUrlEncoded(bytes) {
    return core.opSync("op_url_parse_search_params", null, bytes);
  }

  webidl
    .converters[
      "sequence<sequence<USVString>> or record<USVString, USVString> or USVString"
    ] = (V, opts) => {
      // Union for (sequence<sequence<USVString>> or record<USVString, USVString> or USVString)
      if (webidl.type(V) === "Object" && V !== null) {
        if (V[Symbol.iterator] !== undefined) {
          return webidl.converters["sequence<sequence<ByteString>>"](V, opts);
        }
        return webidl.converters["record<ByteString, ByteString>"](V, opts);
      }
      return webidl.converters.USVString(V, opts);
    };

  window.__bootstrap.url = {
    URL,
    URLSearchParams,
    parseUrlEncoded,
  };
})(this);
