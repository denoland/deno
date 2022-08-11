// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_url.d.ts" />

"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const webidl = window.__bootstrap.webidl;
  const {
    ArrayPrototypeMap,
    ObjectKeys,
    ObjectFromEntries,
    RegExp,
    RegExpPrototypeExec,
    RegExpPrototypeTest,
    Symbol,
    SymbolFor,
    TypeError,
  } = window.__bootstrap.primordials;

  const _components = Symbol("components");

  /**
   * @typedef Components
   * @property {Component} protocol
   * @property {Component} username
   * @property {Component} password
   * @property {Component} hostname
   * @property {Component} port
   * @property {Component} pathname
   * @property {Component} search
   * @property {Component} hash
   */

  /**
   * @typedef Component
   * @property {string} patternString
   * @property {RegExp} regexp
   * @property {string[]} groupNameList
   */

  class URLPattern {
    /** @type {Components} */
    [_components];

    /**
     * @param {URLPatternInput} input
     * @param {string} [baseURL]
     */
    constructor(input, baseURL = undefined) {
      this[webidl.brand] = webidl.brand;
      const prefix = "Failed to construct 'URLPattern'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      input = webidl.converters.URLPatternInput(input, {
        prefix,
        context: "Argument 1",
      });
      if (baseURL !== undefined) {
        baseURL = webidl.converters.USVString(baseURL, {
          prefix,
          context: "Argument 2",
        });
      }

      const components = ops.op_urlpattern_parse(input, baseURL);

      for (const key of ObjectKeys(components)) {
        try {
          components[key].regexp = new RegExp(
            components[key].regexpString,
            "u",
          );
        } catch (e) {
          throw new TypeError(`${prefix}: ${key} is invalid; ${e.message}`);
        }
      }

      this[_components] = components;
    }

    get protocol() {
      webidl.assertBranded(this, URLPatternPrototype);
      return this[_components].protocol.patternString;
    }

    get username() {
      webidl.assertBranded(this, URLPatternPrototype);
      return this[_components].username.patternString;
    }

    get password() {
      webidl.assertBranded(this, URLPatternPrototype);
      return this[_components].password.patternString;
    }

    get hostname() {
      webidl.assertBranded(this, URLPatternPrototype);
      return this[_components].hostname.patternString;
    }

    get port() {
      webidl.assertBranded(this, URLPatternPrototype);
      return this[_components].port.patternString;
    }

    get pathname() {
      webidl.assertBranded(this, URLPatternPrototype);
      return this[_components].pathname.patternString;
    }

    get search() {
      webidl.assertBranded(this, URLPatternPrototype);
      return this[_components].search.patternString;
    }

    get hash() {
      webidl.assertBranded(this, URLPatternPrototype);
      return this[_components].hash.patternString;
    }

    /**
     * @param {URLPatternInput} input
     * @param {string} [baseURL]
     * @returns {boolean}
     */
    test(input, baseURL = undefined) {
      webidl.assertBranded(this, URLPatternPrototype);
      const prefix = "Failed to execute 'test' on 'URLPattern'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      input = webidl.converters.URLPatternInput(input, {
        prefix,
        context: "Argument 1",
      });
      if (baseURL !== undefined) {
        baseURL = webidl.converters.USVString(baseURL, {
          prefix,
          context: "Argument 2",
        });
      }

      const res = ops.op_urlpattern_process_match_input(
        input,
        baseURL,
      );
      if (res === null) {
        return false;
      }

      const [values] = res;

      for (const key of ObjectKeys(values)) {
        if (!RegExpPrototypeTest(this[_components][key].regexp, values[key])) {
          return false;
        }
      }

      return true;
    }

    /**
     * @param {URLPatternInput} input
     * @param {string} [baseURL]
     * @returns {URLPatternResult | null}
     */
    exec(input, baseURL = undefined) {
      webidl.assertBranded(this, URLPatternPrototype);
      const prefix = "Failed to execute 'exec' on 'URLPattern'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      input = webidl.converters.URLPatternInput(input, {
        prefix,
        context: "Argument 1",
      });
      if (baseURL !== undefined) {
        baseURL = webidl.converters.USVString(baseURL, {
          prefix,
          context: "Argument 2",
        });
      }

      const res = ops.op_urlpattern_process_match_input(
        input,
        baseURL,
      );
      if (res === null) {
        return null;
      }

      const [values, inputs] = res;
      if (inputs[1] === null) {
        inputs.pop();
      }

      /** @type {URLPatternResult} */
      const result = { inputs };

      /** @type {string} */
      for (const key of ObjectKeys(values)) {
        /** @type {Component} */
        const component = this[_components][key];
        const input = values[key];
        const match = RegExpPrototypeExec(component.regexp, input);
        if (match === null) {
          return null;
        }
        const groupEntries = ArrayPrototypeMap(
          component.groupNameList,
          (name, i) => [name, match[i + 1] ?? ""],
        );
        const groups = ObjectFromEntries(groupEntries);
        result[key] = {
          input,
          groups,
        };
      }

      return result;
    }

    [SymbolFor("Deno.customInspect")](inspect) {
      return `URLPattern ${
        inspect({
          protocol: this.protocol,
          username: this.username,
          password: this.password,
          hostname: this.hostname,
          port: this.port,
          pathname: this.pathname,
          search: this.search,
          hash: this.hash,
        })
      }`;
    }
  }

  webidl.configurePrototype(URLPattern);
  const URLPatternPrototype = URLPattern.prototype;

  webidl.converters.URLPatternInit = webidl
    .createDictionaryConverter("URLPatternInit", [
      { key: "protocol", converter: webidl.converters.USVString },
      { key: "username", converter: webidl.converters.USVString },
      { key: "password", converter: webidl.converters.USVString },
      { key: "hostname", converter: webidl.converters.USVString },
      { key: "port", converter: webidl.converters.USVString },
      { key: "pathname", converter: webidl.converters.USVString },
      { key: "search", converter: webidl.converters.USVString },
      { key: "hash", converter: webidl.converters.USVString },
      { key: "baseURL", converter: webidl.converters.USVString },
    ]);

  webidl.converters["URLPatternInput"] = (V, opts) => {
    // Union for (URLPatternInit or USVString)
    if (typeof V == "object") {
      return webidl.converters.URLPatternInit(V, opts);
    }
    return webidl.converters.USVString(V, opts);
  };

  window.__bootstrap.urlPattern = {
    URLPattern,
  };
})(globalThis);
