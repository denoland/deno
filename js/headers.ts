// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";
import { DomIterableMixin } from "./mixins/dom_iterable";
import { requiredArguments } from "./util";

// From node-fetch
// Copyright (c) 2016 David Frank. MIT License.
const invalidTokenRegex = /[^\^_`a-zA-Z\-0-9!#$%&'*+.|~]/;
const invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/;

// tslint:disable-next-line:no-any
function isHeaders(value: any): value is domTypes.Headers {
  return value instanceof Headers;
}

const headerMap = Symbol("header map");

// ref: https://fetch.spec.whatwg.org/#dom-headers
class HeadersBase {
  private [headerMap]: Map<string, string>;
  // TODO: headerGuard? Investigate if it is needed
  // node-fetch did not implement this but it is in the spec

  private _normalizeParams(name: string, value?: string): string[] {
    name = String(name).toLowerCase();
    value = String(value).trim();
    return [name, value];
  }

  // The following name/value validations are copied from
  // https://github.com/bitinn/node-fetch/blob/master/src/headers.js
  // Copyright (c) 2016 David Frank. MIT License.
  private _validateName(name: string) {
    if (invalidTokenRegex.test(name) || name === "") {
      throw new TypeError(`${name} is not a legal HTTP header name`);
    }
  }

  private _validateValue(value: string) {
    if (invalidHeaderCharRegex.test(value)) {
      throw new TypeError(`${value} is not a legal HTTP header value`);
    }
  }

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
          if (tuple.length !== 2) {
            // tslint:disable:max-line-length
            // prettier-ignore
            throw new TypeError("Failed to construct 'Headers'; Each header pair must be an iterable [name, value] tuple");
          }

          const [name, value] = this._normalizeParams(tuple[0], tuple[1]);
          this._validateName(name);
          this._validateValue(value);
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
          const [name, value] = this._normalizeParams(rawName, rawValue);
          this._validateName(name);
          this._validateValue(value);
          this[headerMap].set(name, value);
        }
      }
    }
  }

  // ref: https://fetch.spec.whatwg.org/#concept-headers-append
  append(name: string, value: string): void {
    requiredArguments("Headers.append", arguments.length, 2);
    const [newname, newvalue] = this._normalizeParams(name, value);
    this._validateName(newname);
    this._validateValue(newvalue);
    const v = this[headerMap].get(newname);
    const str = v ? `${v}, ${newvalue}` : newvalue;
    this[headerMap].set(newname, str);
  }

  delete(name: string): void {
    requiredArguments("Headers.delete", arguments.length, 1);
    const [newname] = this._normalizeParams(name);
    this._validateName(newname);
    this[headerMap].delete(newname);
  }

  get(name: string): string | null {
    requiredArguments("Headers.get", arguments.length, 1);
    const [newname] = this._normalizeParams(name);
    this._validateName(newname);
    const value = this[headerMap].get(newname);
    return value || null;
  }

  has(name: string): boolean {
    requiredArguments("Headers.has", arguments.length, 1);
    const [newname] = this._normalizeParams(name);
    this._validateName(newname);
    return this[headerMap].has(newname);
  }

  set(name: string, value: string): void {
    requiredArguments("Headers.set", arguments.length, 2);
    const [newname, newvalue] = this._normalizeParams(name, value);
    this._validateName(newname);
    this._validateValue(newvalue);
    this[headerMap].set(newname, newvalue);
  }
}

// @internal
// tslint:disable-next-line:variable-name
export class Headers extends DomIterableMixin<
  string,
  string,
  typeof HeadersBase
>(HeadersBase, headerMap) {}
