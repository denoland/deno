// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import { DomIterableMixin } from "./dom_iterable.ts";
import { requiredArguments } from "./util.ts";
import { customInspect } from "./console.ts";

// From node-fetch
// Copyright (c) 2016 David Frank. MIT License.
const invalidTokenRegex = /[^\^_`a-zA-Z\-0-9!#$%&'*+.|~]/;
const invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/;

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function isHeaders(value: any): value is domTypes.Headers {
  // eslint-disable-next-line @typescript-eslint/no-use-before-define
  return value instanceof Headers;
}

const headerMap = Symbol("header map");

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

  constructor(init?: domTypes.HeadersInit) {
    if (init === null) {
      throw new TypeError(
        "Failed to construct 'Headers'; The provided value was not valid"
      );
    } else if (isHeaders(init)) {
      this[headerMap] = new Map(init);
    } else {
      this[headerMap] = new Map();
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

          const [name, value] = normalizeParams(tuple[0], tuple[1]);
          validateName(name);
          validateValue(value);
          const existingValue = this[headerMap].get(name);
          this[headerMap].set(
            name,
            existingValue ? `${existingValue}, ${value}` : value
          );
        }
      } else if (init) {
        const names = Object.keys(init);
        for (const rawName of names) {
          const rawValue = init[rawName];
          const [name, value] = normalizeParams(rawName, rawValue);
          validateName(name);
          validateValue(value);
          this[headerMap].set(name, value);
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

  // ref: https://fetch.spec.whatwg.org/#concept-headers-append
  append(name: string, value: string): void {
    requiredArguments("Headers.append", arguments.length, 2);
    const [newname, newvalue] = normalizeParams(name, value);
    validateName(newname);
    validateValue(newvalue);
    const v = this[headerMap].get(newname);
    const str = v ? `${v}, ${newvalue}` : newvalue;
    this[headerMap].set(newname, str);
  }

  delete(name: string): void {
    requiredArguments("Headers.delete", arguments.length, 1);
    const [newname] = normalizeParams(name);
    validateName(newname);
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
    this[headerMap].set(newname, newvalue);
  }

  get [Symbol.toStringTag](): string {
    return "Headers";
  }
}

// @internal
export class Headers extends DomIterableMixin<
  string,
  string,
  typeof HeadersBase
>(HeadersBase, headerMap) {}
