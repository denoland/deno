// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;

  function requiredArguments(name, length, required) {
    if (length < required) {
      const errMsg = `${name} requires at least ${required} argument${
        required === 1 ? "" : "s"
      }, but only ${length} present`;
      throw new TypeError(errMsg);
    }
  }

  const paramLists = new WeakMap();
  const urls = new WeakMap();

  class URLSearchParams {
    #params = [];

    constructor(init = "") {
      if (typeof init === "string") {
        // Overload: USVString
        // If init is a string and starts with U+003F (?),
        // remove the first code point from init.
        if (init[0] == "?") {
          init = init.slice(1);
        }

        this.#params = core.jsonOpSync("op_parse_url_search_params", init);
      } else if (
        Array.isArray(init) ||
        typeof init?.[Symbol.iterator] == "function"
      ) {
        // Overload: sequence<sequence<USVString>>
        for (const pair of init) {
          // If pair does not contain exactly two items, then throw a TypeError.
          if (pair.length !== 2) {
            throw new TypeError(
              "URLSearchParams.constructor sequence argument must only contain pair elements",
            );
          }
          this.#params.push([String(pair[0]), String(pair[1])]);
        }
      } else if (Object(init) !== init) {
        // pass
      } else if (init instanceof URLSearchParams) {
        this.#params = [...init.#params];
      } else {
        // Overload: record<USVString, USVString>
        for (const key of Object.keys(init)) {
          this.#params.push([key, String(init[key])]);
        }
      }

      paramLists.set(this, this.#params);
      urls.set(this, null);
    }

    #updateUrlSearch = () => {
      const url = urls.get(this);
      if (url == null) {
        return;
      }
      const parseArgs = { href: url.href, setSearch: this.toString() };
      parts.set(url, core.jsonOpSync("op_parse_url", parseArgs));
    };

    append(name, value) {
      requiredArguments("URLSearchParams.append", arguments.length, 2);
      this.#params.push([String(name), String(value)]);
      this.#updateUrlSearch();
    }

    delete(name) {
      requiredArguments("URLSearchParams.delete", arguments.length, 1);
      name = String(name);
      let i = 0;
      while (i < this.#params.length) {
        if (this.#params[i][0] === name) {
          this.#params.splice(i, 1);
        } else {
          i++;
        }
      }
      this.#updateUrlSearch();
    }

    getAll(name) {
      requiredArguments("URLSearchParams.getAll", arguments.length, 1);
      name = String(name);
      const values = [];
      for (const entry of this.#params) {
        if (entry[0] === name) {
          values.push(entry[1]);
        }
      }

      return values;
    }

    get(name) {
      requiredArguments("URLSearchParams.get", arguments.length, 1);
      name = String(name);
      for (const entry of this.#params) {
        if (entry[0] === name) {
          return entry[1];
        }
      }

      return null;
    }

    has(name) {
      requiredArguments("URLSearchParams.has", arguments.length, 1);
      name = String(name);
      return this.#params.some((entry) => entry[0] === name);
    }

    set(name, value) {
      requiredArguments("URLSearchParams.set", arguments.length, 2);

      // If there are any name-value pairs whose name is name, in list,
      // set the value of the first such name-value pair to value
      // and remove the others.
      name = String(name);
      value = String(value);
      let found = false;
      let i = 0;
      while (i < this.#params.length) {
        if (this.#params[i][0] === name) {
          if (!found) {
            this.#params[i][1] = value;
            found = true;
            i++;
          } else {
            this.#params.splice(i, 1);
          }
        } else {
          i++;
        }
      }

      // Otherwise, append a new name-value pair whose name is name
      // and value is value, to list.
      if (!found) {
        this.#params.push([String(name), String(value)]);
      }

      this.#updateUrlSearch();
    }

    sort() {
      this.#params.sort((a, b) => (a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1));
      this.#updateUrlSearch();
    }

    forEach(callbackfn, thisArg) {
      requiredArguments("URLSearchParams.forEach", arguments.length, 1);

      if (typeof thisArg !== "undefined") {
        callbackfn = callbackfn.bind(thisArg);
      }

      for (const [key, value] of this.#params) {
        callbackfn(value, key, this);
      }
    }

    *keys() {
      for (const [key] of this.#params) {
        yield key;
      }
    }

    *values() {
      for (const [, value] of this.#params) {
        yield value;
      }
    }

    *entries() {
      yield* this.#params;
    }

    *[Symbol.iterator]() {
      yield* this.#params;
    }

    toString() {
      return core.jsonOpSync("op_stringify_url_search_params", this.#params);
    }
  }

  const parts = new WeakMap();

  class URL {
    #searchParams = null;

    constructor(url, base) {
      new.target;

      if (url instanceof URL && base === undefined) {
        parts.set(this, parts.get(url));
      } else {
        base = base !== undefined ? String(base) : base;
        const parseArgs = { href: String(url), baseHref: base };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      }
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

    #updateSearchParams = () => {
      if (this.#searchParams != null) {
        const params = paramLists.get(this.#searchParams);
        const newParams = core.jsonOpSync(
          "op_parse_url_search_params",
          this.search.slice(1),
        );
        params.splice(0, params.length, ...newParams);
      }
    };

    get hash() {
      return parts.get(this).hash;
    }

    set hash(value) {
      try {
        const parseArgs = { href: this.href, setHash: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      } catch {
        /* pass */
      }
    }

    get host() {
      return parts.get(this).host;
    }

    set host(value) {
      try {
        const parseArgs = { href: this.href, setHost: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      } catch {
        /* pass */
      }
    }

    get hostname() {
      return parts.get(this).hostname;
    }

    set hostname(value) {
      try {
        const parseArgs = { href: this.href, setHostname: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      } catch {
        /* pass */
      }
    }

    get href() {
      return parts.get(this).href;
    }

    set href(value) {
      try {
        const parseArgs = { href: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      } catch {
        throw new TypeError("Invalid URL");
      }
      this.#updateSearchParams();
    }

    get origin() {
      return parts.get(this).origin;
    }

    get password() {
      return parts.get(this).password;
    }

    set password(value) {
      try {
        const parseArgs = { href: this.href, setPassword: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      } catch {
        /* pass */
      }
    }

    get pathname() {
      return parts.get(this).pathname;
    }

    set pathname(value) {
      try {
        const parseArgs = { href: this.href, setPathname: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      } catch {
        /* pass */
      }
    }

    get port() {
      return parts.get(this).port;
    }

    set port(value) {
      try {
        const parseArgs = { href: this.href, setPort: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      } catch {
        /* pass */
      }
    }

    get protocol() {
      return parts.get(this).protocol;
    }

    set protocol(value) {
      try {
        const parseArgs = { href: this.href, setProtocol: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      } catch {
        /* pass */
      }
    }

    get search() {
      return parts.get(this).search;
    }

    set search(value) {
      try {
        const parseArgs = { href: this.href, setSearch: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
        this.#updateSearchParams();
      } catch {
        /* pass */
      }
    }

    get username() {
      return parts.get(this).username;
    }

    set username(value) {
      try {
        const parseArgs = { href: this.href, setUsername: String(value) };
        parts.set(this, core.jsonOpSync("op_parse_url", parseArgs));
      } catch {
        /* pass */
      }
    }

    get searchParams() {
      if (this.#searchParams == null) {
        this.#searchParams = new URLSearchParams(this.search);
        urls.set(this.#searchParams, this);
      }
      return this.#searchParams;
    }

    toString() {
      return this.href;
    }

    toJSON() {
      return this.href;
    }

    static createObjectURL() {
      throw new Error("Not implemented");
    }

    static revokeObjectURL() {
      throw new Error("Not implemented");
    }
  }

  window.__bootstrap.url = {
    URL,
    URLSearchParams,
  };
})(this);
