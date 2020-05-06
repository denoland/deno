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

function dataAppend(
  data: Array<[string, string]>,
  key: string,
  value: string
): void {
  if (key === "set-cookie") {
    data.push([key, value]);
    return;
  }
  for (let i = 0; i < data.length; i++) {
    const [dataKey] = data[i];
    if (dataKey === key) {
      data[i][1] += `, ${value}`;
      return;
    }
  }
  data.push([key, value]);
}

function dataGet(
  data: Array<[string, string]>,
  key: string
): string | undefined {
  for (const [dataKey, value] of data) {
    if (dataKey === key) {
      return value;
    }
  }
  return undefined;
}

function dataSet(
  data: Array<[string, string]>,
  key: string,
  value: string
): void {
  let found = false;
  for (let i = 0; i < data.length; i++) {
    const [dataKey] = data[i];
    if (dataKey === key) {
      data[i][1] = value;
      // there could be multiple set-cookie headers, but all others are unique
      if (key !== "set-cookie") {
        return;
      } else {
        found = true;
      }
    }
  }
  if (!found) {
    data.push([key, value]);
  }
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
    if (init === null) {
      throw new TypeError(
        "Failed to construct 'Headers'; The provided value was not valid"
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
            2
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
    const [newname, newvalue] = normalizeParams(name, value);
    validateName(newname);
    validateValue(newvalue);
    dataAppend(this[headersData], newname, newvalue);
  }

  delete(name: string): void {
    requiredArguments("Headers.delete", arguments.length, 1);
    const [newname] = normalizeParams(name);
    validateName(newname);
    dataDelete(this[headersData], newname);
  }

  get(name: string): string | null {
    requiredArguments("Headers.get", arguments.length, 1);
    const [newname] = normalizeParams(name);
    validateName(newname);
    return dataGet(this[headersData], newname) ?? null;
  }

  has(name: string): boolean {
    requiredArguments("Headers.has", arguments.length, 1);
    const [newname] = normalizeParams(name);
    validateName(newname);
    return dataHas(this[headersData], newname);
  }

  set(name: string, value: string): void {
    requiredArguments("Headers.set", arguments.length, 2);
    const [newname, newvalue] = normalizeParams(name, value);
    validateName(newname);
    validateValue(newvalue);
    dataSet(this[headersData], newname, newvalue);
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
