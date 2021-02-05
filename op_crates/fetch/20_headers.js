// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const { DomIterableMixin } = window.__bootstrap.domIterable;
  const { requiredArguments } = window.__bootstrap.fetchUtil;

  // From node-fetch
  // Copyright (c) 2016 David Frank. MIT License.
  const invalidTokenRegex = /[^\^_`a-zA-Z\-0-9!#$%&'*+.|~]/;
  const invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/;

  function isHeaders(value) {
    // eslint-disable-next-line @typescript-eslint/no-use-before-define
    return value instanceof Headers;
  }

  const headersData = Symbol("headers data");

  // TODO(bartlomieju): headerGuard? Investigate if it is needed
  // node-fetch did not implement this but it is in the spec
  function normalizeParams(name, value) {
    name = String(name).toLowerCase();
    value = String(value).trim();
    return [name, value];
  }

  // The following name/value validations are copied from
  // https://github.com/bitinn/node-fetch/blob/master/src/headers.js
  // Copyright (c) 2016 David Frank. MIT License.
  function validateName(name) {
    if (invalidTokenRegex.test(name) || name === "") {
      throw new TypeError(`${name} is not a legal HTTP header name`);
    }
  }

  function validateValue(value) {
    if (invalidHeaderCharRegex.test(value)) {
      throw new TypeError(`${value} is not a legal HTTP header value`);
    }
  }

  /** Appends a key and value to the header list.
 *
 * The spec indicates that when a key already exists, the append adds the new
 * value onto the end of the existing value.  The behaviour of this though
 * varies when the key is `set-cookie`.  In this case, if the key of the cookie
 * already exists, the value is replaced, but if the key of the cookie does not
 * exist, and additional `set-cookie` header is added.
 *
 * The browser specification of `Headers` is written for clients, and not
 * servers, and Deno is a server, meaning that it needs to follow the patterns
 * expected for servers, of which a `set-cookie` header is expected for each
 * unique cookie key, but duplicate cookie keys should not exist. */
  function dataAppend(
    data,
    key,
    value,
  ) {
    for (let i = 0; i < data.length; i++) {
      const [dataKey] = data[i];
      if (key === "set-cookie" && dataKey === "set-cookie") {
        const [, dataValue] = data[i];
        const [dataCookieKey] = dataValue.split("=");
        const [cookieKey] = value.split("=");
        if (dataCookieKey === cookieKey) {
          data[i][1] = value;
          return;
        }
      } else {
        if (dataKey === key) {
          data[i][1] += `, ${value}`;
          return;
        }
      }
    }
    data.push([key, value]);
  }

  /** Gets a value of a key in the headers list.
 *
 * This varies slightly from spec behaviour in that when the key is `set-cookie`
 * the value returned will look like a concatenated value, when in fact, if the
 * headers were iterated over, each individual `set-cookie` value is a unique
 * entry in the headers list. */
  function dataGet(
    data,
    key,
  ) {
    const setCookieValues = [];
    for (const [dataKey, value] of data) {
      if (dataKey === key) {
        if (key === "set-cookie") {
          setCookieValues.push(value);
        } else {
          return value;
        }
      }
    }
    if (setCookieValues.length) {
      return setCookieValues.join(", ");
    }
    return undefined;
  }

  /** Sets a value of a key in the headers list.
 *
 * The spec indicates that the value should be replaced if the key already
 * exists.  The behaviour here varies, where if the key is `set-cookie` the key
 * of the cookie is inspected, and if the key of the cookie already exists,
 * then the value is replaced.  If the key of the cookie is not found, then
 * the value of the `set-cookie` is added to the list of headers.
 *
 * The browser specification of `Headers` is written for clients, and not
 * servers, and Deno is a server, meaning that it needs to follow the patterns
 * expected for servers, of which a `set-cookie` header is expected for each
 * unique cookie key, but duplicate cookie keys should not exist. */
  function dataSet(
    data,
    key,
    value,
  ) {
    for (let i = 0; i < data.length; i++) {
      const [dataKey] = data[i];
      if (dataKey === key) {
        // there could be multiple set-cookie headers, but all others are unique
        if (key === "set-cookie") {
          const [, dataValue] = data[i];
          const [dataCookieKey] = dataValue.split("=");
          const [cookieKey] = value.split("=");
          if (cookieKey === dataCookieKey) {
            data[i][1] = value;
            return;
          }
        } else {
          data[i][1] = value;
          return;
        }
      }
    }
    data.push([key, value]);
  }

  function dataDelete(data, key) {
    let i = 0;
    while (i < data.length) {
      const [dataKey] = data[i];
      if (dataKey === key) {
        data.splice(i, 1);
      } else {
        i++;
      }
    }
  }

  function dataHas(data, key) {
    for (const [dataKey] of data) {
      if (dataKey === key) {
        return true;
      }
    }
    return false;
  }

  // ref: https://fetch.spec.whatwg.org/#dom-headers
  class HeadersBase {
    constructor(init) {
      if (init === null) {
        throw new TypeError(
          "Failed to construct 'Headers'; The provided value was not valid",
        );
      } else if (isHeaders(init)) {
        this[headersData] = [...init];
      } else {
        this[headersData] = [];
        if (Array.isArray(init)) {
          for (const tuple of init) {
            // If header does not contain exactly two items,
            // then throw a TypeError.
            // ref: https://fetch.spec.whatwg.org/#concept-headers-fill
            requiredArguments(
              "Headers.constructor tuple array argument",
              tuple.length,
              2,
            );

            this.append(tuple[0], tuple[1]);
          }
        } else if (init) {
          for (const [rawName, rawValue] of Object.entries(init)) {
            this.append(rawName, rawValue);
          }
        }
      }
    }

    [Symbol.for("Deno.customInspect")]() {
      let length = this[headersData].length;
      let output = "";
      for (const [key, value] of this[headersData]) {
        const prefix = length === this[headersData].length ? " " : "";
        const postfix = length === 1 ? " " : ", ";
        output = output + `${prefix}${key}: ${value}${postfix}`;
        length--;
      }
      return `Headers {${output}}`;
    }

    // ref: https://fetch.spec.whatwg.org/#concept-headers-append
    append(name, value) {
      requiredArguments("Headers.append", arguments.length, 2);
      const [newname, newvalue] = normalizeParams(name, value);
      validateName(newname);
      validateValue(newvalue);
      dataAppend(this[headersData], newname, newvalue);
    }

    delete(name) {
      requiredArguments("Headers.delete", arguments.length, 1);
      const [newname] = normalizeParams(name);
      validateName(newname);
      dataDelete(this[headersData], newname);
    }

    get(name) {
      requiredArguments("Headers.get", arguments.length, 1);
      const [newname] = normalizeParams(name);
      validateName(newname);
      return dataGet(this[headersData], newname) ?? null;
    }

    has(name) {
      requiredArguments("Headers.has", arguments.length, 1);
      const [newname] = normalizeParams(name);
      validateName(newname);
      return dataHas(this[headersData], newname);
    }

    set(name, value) {
      requiredArguments("Headers.set", arguments.length, 2);
      const [newName, newValue] = normalizeParams(name, value);
      validateName(newName);
      validateValue(newValue);
      dataSet(this[headersData], newName, newValue);
    }

    get [Symbol.toStringTag]() {
      return "Headers";
    }
  }

  class Headers extends DomIterableMixin(HeadersBase, headersData) {}

  window.__bootstrap.headers = {
    Headers,
  };
})(this);
