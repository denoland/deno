// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { DomIterableMixin } from "./dom_iterable.ts";
import { requiredArguments } from "./util.ts";
import { customInspect } from "./console.ts";

// From node-fetch
// Copyright (c) 2016 David Frank. MIT License.
const invalidTokenRegex = /[^\^_`a-zA-Z\-0-9!#$%&'*+.|~]/;
const invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function isHeaders(value: any): value is Headers {
  // eslint-disable-next-line @typescript-eslint/no-use-before-define
  return value instanceof Headers;
}

const headersData = Symbol("headers data");

// TODO: headerGuard? Investigate if it is needed
// node-fetch did not implement this but it is in the spec
function normalizeParams(name: string, value?: string): string[] {
  return [String(name).toLowerCase(), String(value).trim()];
}

// The following name/value validations are copied from
// https://github.com/bitinn/node-fetch/blob/master/src/headers.js
// Copyright (c) 2016 David Frank. MIT License.
function validateName(name: string): void {
  if (invalidTokenRegex.test(name) || name === "") {
    throw new TypeError(`${name} is not a legal HTTP header name`);
  }
}

function validateValue(value: string): void {
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
  data: Array<[string, string]>,
  key: string,
  value: string
): void {
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
  data: Array<[string, string]>,
  key: string
): string | undefined {
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
  data: Array<[string, string]>,
  key: string,
  value: string
): void {
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

function dataDelete(data: Array<[string, string]>, key: string): void {
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

function dataHas(data: Array<[string, string]>, key: string): boolean {
  for (const [dataKey] of data) {
    if (dataKey === key) {
      return true;
    }
  }
  return false;
}

// ref: https://fetch.spec.whatwg.org/#dom-headers
class HeadersBase {
  [headersData]: Array<[string, string]>;

  constructor(init?: HeadersInit) {
    if (init == null) {
      throw new TypeError(
        "Failed to construct 'Headers'; The provided value was not valid"
      );
    }

    if (isHeaders(init)) {
      this[headersData] = [...init];
    }

    this[headersData] = [];

    if (Array.isArray(init)) {
      for (const tuple of init) {
        // If header does not contain exactly two items,
        // then throw a TypeError.
        // ref: https://fetch.spec.whatwg.org/#concept-headers-fill
        requiredArguments(
          "Headers.constructor tuple array argument",
          tuple.length,
          2
        );

        this.append(tuple[0], tuple[1]);
      }
    }

    for (const [key, value] of Object.entries(init)) {
      this.append(key, value);
    }
  }

  [customInspect](): string {
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
  append(name: string, value: string): void {
    requiredArguments("Headers.append", arguments.length, 2);
    const [newName, newValue] = normalizeParams(name, value);
    validateName(newName);
    validateValue(newValue);
    dataAppend(this[headersData], newName, newValue);
  }

  delete(name: string): void {
    requiredArguments("Headers.delete", arguments.length, 1);
    const newName = normalizeParams(name)[0];
    validateName(newName);
    dataDelete(this[headersData], newName);
  }

  get(name: string): string | null {
    requiredArguments("Headers.get", arguments.length, 1);
    const newName = normalizeParams(name)[0];
    validateName(newName);
    return dataGet(this[headersData], newName) ?? null;
  }

  has(name: string): boolean {
    requiredArguments("Headers.has", arguments.length, 1);
    const newName = normalizeParams(name)[0];
    validateName(newName);
    return dataHas(this[headersData], newName);
  }

  set(name: string, value: string): void {
    requiredArguments("Headers.set", arguments.length, 2);
    const [newName, newValue] = normalizeParams(name, value);
    validateName(newName);
    validateValue(newValue);
    dataSet(this[headersData], newName, newValue);
  }

  get [Symbol.toStringTag](): string {
    return "Headers";
  }
}

// @internal
export class HeadersImpl extends DomIterableMixin<
  string,
  string,
  typeof HeadersBase
>(HeadersBase, headersData) {}

Object.defineProperty(HeadersImpl, "name", {
  value: "Headers",
  configurable: true,
});
