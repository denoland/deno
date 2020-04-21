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

const headerMap = Symbol("header map");
const cookieMap = Symbol("cookie map");

// TODO: headerGuard? Investigate if it is needed
// node-fetch did not implement this but it is in the spec
function normalizeParams(name: string, value?: string): string[] {
  name = String(name).toLowerCase();
  value = String(value).trim();
  return [name, value];
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

// ref: https://fetch.spec.whatwg.org/#dom-headers
class HeadersBase {
  [headerMap]: Map<string, string>;

  // https://tools.ietf.org/html/rfc6265#section-4.1.1
  // Servers SHOULD NOT include more than one Set-Cookie header field in
  // the same response with the same cookie-name
  [cookieMap]: Map<string, string>;

  constructor(init?: HeadersInit) {
    if (init === null) {
      throw new TypeError(
        "Failed to construct 'Headers'; The provided value was not valid"
      );
    } else if (isHeaders(init)) {
      this[headerMap] = new Map(init);
      // @ts-ignore
      this[cookieMap] = init[cookieMap] || new Map();
    } else {
      this[headerMap] = new Map();
      this[cookieMap] = new Map();
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
      } else if (init) {
        const names = Object.keys(init);
        for (const rawName of names) {
          const rawValue = init[rawName];
          this.set(rawName, rawValue);
        }
      }
    }
  }

  [customInspect](): string {
    let headerSize = this[headerMap].size;
    let output = "";
    this[headerMap].forEach((value, key) => {
      const prefix = headerSize === this[headerMap].size ? " " : "";
      const postfix = headerSize === 1 ? " " : ", ";
      output = output + `${prefix}${key}: ${value}${postfix}`;
      headerSize--;
    });
    return `Headers {${output}}`;
  }

  cookies() {
    return this[cookieMap].values();
  }

  // ref: https://fetch.spec.whatwg.org/#concept-headers-append
  append(name: string, value: string): void {
    requiredArguments("Headers.append", arguments.length, 2);
    const [newname, newvalue] = normalizeParams(name, value);
    validateName(newname);
    validateValue(newvalue);

    if (newname === "set-cookie") {
      const [cookieName] = newvalue.split("=");
      this[cookieMap].set(cookieName, newvalue);
    }

    const v = this[headerMap].get(newname);
    const str = v ? `${v}, ${newvalue}` : newvalue;
    this[headerMap].set(newname, str);
  }

  delete(name: string): void {
    requiredArguments("Headers.delete", arguments.length, 1);
    const [newname] = normalizeParams(name);
    validateName(newname);

    if (newname === "set-cookie") {
      this[cookieMap].clear();
    }

    this[headerMap].delete(newname);
  }

  get(name: string): string | null {
    requiredArguments("Headers.get", arguments.length, 1);
    const [newname] = normalizeParams(name);
    validateName(newname);
    const value = this[headerMap].get(newname);
    return value || null;
  }

  has(name: string): boolean {
    requiredArguments("Headers.has", arguments.length, 1);
    const [newname] = normalizeParams(name);
    validateName(newname);
    return this[headerMap].has(newname);
  }

  set(name: string, value: string): void {
    requiredArguments("Headers.set", arguments.length, 2);
    const [newname, newvalue] = normalizeParams(name, value);
    validateName(newname);
    validateValue(newvalue);

    if (newname === "set-cookie") {
      const [cookieName] = newvalue.split("=");
      this[cookieMap].set(cookieName, newvalue);
    }

    this[headerMap].set(newname, newvalue);
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
>(HeadersBase, headerMap) {}
