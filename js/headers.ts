// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { requiredArguments } from "./util";

// From node-fetch
// Copyright (c) 2016 David Frank. MIT License.
const invalidTokenRegex = /[^\^_`a-zA-Z\-0-9!#$%&'*+.|~]/;
const invalidHeaderCharRegex = /[^\t\x20-\x7e\x80-\xff]/;

const entries = Symbol("entries");

// ref: https://fetch.spec.whatwg.org/#dom-headers
export class Headers {
  private [entries]: Array<[string, string]>;

  private _normalizeParams(name: string, value: string): string[] {
    return [this._normalizeName(name), this._normalizeValue(value)];
  }

  private _normalizeName(name: string): string {
    return String(name).toLowerCase();
  }

  private _normalizeValue(value: string): string {
    return String(value).trim();
  }

  // The following name/value validations are copied from
  // https://github.com/bitinn/node-fetch/blob/master/src/headers.js
  // Copyright (c) 2016 David Frank. MIT License.
  private _validateName(name: string): void {
    if (invalidTokenRegex.test(name) || name === "") {
      throw new TypeError(`${name} is not a legal HTTP header name`);
    }
  }

  private _validateValue(value: string): void {
    if (invalidHeaderCharRegex.test(value)) {
      throw new TypeError(`${value} is not a legal HTTP header value`);
    }
  }

  constructor(init?: Array<[string, string]> | object) {
    this[entries] = [];
    if (init === null) {
      throw new TypeError(
        "Failed to construct 'Headers'; The provided value was not valid"
      );
    }
    // Object type constructors
    if (!Array.isArray(init) && typeof init == "object") {
      for (const [name, value] of Object.entries(init)) {
        const [newname, newvalue] = this._normalizeParams(name, value);
        this._validateName(newname);
        this._validateValue(newvalue);
        this[entries].push([newname, newvalue]);
      }
    }
    // Array type constructors
    else if (Array.isArray(init)) {
      for (const [name, value] of init) {
        const [newname, newvalue] = this._normalizeParams(name, value);
        this._validateName(newname);
        this._validateValue(newvalue);
        this[entries].push([newname, newvalue]);
      }
    }
  }

  [Symbol.iterator](): IterableIterator<[string, string]> {
    return this[entries][Symbol.iterator]();
  }

  // ref: https://fetch.spec.whatwg.org/#concept-headers-append
  append(name: string, value: string): void {
    requiredArguments("Headers.append", arguments.length, 2);
    const [newname, newvalue] = this._normalizeParams(name, value);
    this._validateName(newname);
    this._validateValue(newvalue);
    this[entries].push([newname, newvalue]);
  }

  // https://fetch.spec.whatwg.org/#dom-headers-get
  get(name: string): string | null {
    requiredArguments("Headers.get", arguments.length, 1);
    const newname = this._normalizeName(name);
    this._validateName(newname);
    const matches = this[entries].filter((h): boolean => h[0] == newname);
    if (!matches.length) return null;
    const values = matches.map((m): string => m[1]);
    return values.join(", ");
  }

  // https://fetch.spec.whatwg.org/#dom-headers-has
  has(name: string): boolean {
    requiredArguments("Headers.has", arguments.length, 1);
    const newname = this._normalizeName(name);
    this._validateName(newname);
    const result = this[entries].find(
      (header): boolean => header[0] == newname
    );
    return Boolean(result);
  }

  // https://fetch.spec.whatwg.org/#dom-headers-set
  set(name: string, value: string): void {
    requiredArguments("Headers.set", arguments.length, 2);
    const [newname, newvalue] = this._normalizeParams(name, value);
    this._validateName(newname);
    this._validateValue(newvalue);
    this.delete(newname);
    this[entries].push([newname, newvalue]);
  }

  // https://fetch.spec.whatwg.org/#dom-headers-delete
  delete(name: string): void {
    requiredArguments("Headers.delete", arguments.length, 1);
    const newname = this._normalizeName(name);
    this._validateName(newname);
    this[entries] = this[entries].filter((h): boolean => h[0] !== newname);
  }

  get [Symbol.toStringTag](): string {
    return "Headers";
  }
}
