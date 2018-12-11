// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types";
import { DomIterableMixin } from "./mixins/dom_iterable";

// tslint:disable-next-line:no-any
function isHeaders(value: any): value is domTypes.Headers {
  return value instanceof Headers;
}

const headerMap = Symbol("header map");

// ref: https://fetch.spec.whatwg.org/#dom-headers
class HeadersBase {
  private [headerMap]: Map<string, string>;

  private _normalizeParams(name: string, value?: string): string[] {
    name = String(name).toLowerCase();
    value = String(value).trim();
    return [name, value];
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
        for (const [rawName, rawValue] of init) {
          const [name, value] = this._normalizeParams(rawName, rawValue);
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
          this[headerMap].set(name, value);
        }
      }
    }
  }

  append(name: string, value: string): void {
    const [newname, newvalue] = this._normalizeParams(name, value);
    const v = this[headerMap].get(newname);
    const str = v ? `${v}, ${newvalue}` : newvalue;
    this[headerMap].set(newname, str);
  }

  delete(name: string): void {
    const [newname] = this._normalizeParams(name);
    this[headerMap].delete(newname);
  }

  get(name: string): string | null {
    const [newname] = this._normalizeParams(name);
    const value = this[headerMap].get(newname);
    return value || null;
  }

  has(name: string): boolean {
    const [newname] = this._normalizeParams(name);
    return this[headerMap].has(newname);
  }

  set(name: string, value: string): void {
    const [newname, newvalue] = this._normalizeParams(name, value);
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
