// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as domTypes from "./dom_types.ts";
import { URL, parts } from "./url.ts";
import { isIterable, requiredArguments } from "./util.ts";

/** @internal */
export const urls = new WeakMap<URLSearchParams, URL | null>();

function handleStringInitialization(
  searchParams: URLSearchParams,
  init: string
): void {
  // Overload: USVString
  // If init is a string and starts with U+003F (?),
  // remove the first code point from init.
  if (init.charCodeAt(0) === 0x003f) {
    init = init.slice(1);
  }

  for (const pair of init.split("&")) {
    // Empty params are ignored
    if (pair.length === 0) {
      continue;
    }
    const position = pair.indexOf("=");
    const name = pair.slice(0, position === -1 ? pair.length : position);
    const value = pair.slice(name.length + 1);
    searchParams.append(decodeURIComponent(name), decodeURIComponent(value));
  }
}

function handleArrayInitialization(
  searchParams: URLSearchParams,
  init: string[][] | Iterable<[string, string]>
): void {
  // Overload: sequence<sequence<USVString>>
  for (const tuple of init) {
    // If pair does not contain exactly two items, then throw a TypeError.
    if (tuple.length !== 2) {
      throw new TypeError(
        "URLSearchParams.constructor tuple array argument must only contain pair elements"
      );
    }
    searchParams.append(tuple[0], tuple[1]);
  }
}

export class URLSearchParams implements domTypes.URLSearchParams {
  #params: Array<[string, string]> = [];

  constructor(init: string | string[][] | Record<string, string> = "") {
    if (typeof init === "string") {
      handleStringInitialization(this, init);
      return;
    }

    if (Array.isArray(init) || isIterable(init)) {
      handleArrayInitialization(this, init);
      return;
    }

    if (Object(init) !== init) {
      return;
    }

    if (init instanceof URLSearchParams) {
      this.#params = [...init.#params];
      return;
    }

    // Overload: record<USVString, USVString>
    for (const key of Object.keys(init)) {
      this.append(key, init[key]);
    }

    urls.set(this, null);
  }

  #updateSteps = (): void => {
    const url = urls.get(this);
    if (url == null) {
      return;
    }

    let query: string | null = this.toString();
    if (query === "") {
      query = null;
    }

    parts.get(url)!.query = query;
  };

  append(name: string, value: string): void {
    requiredArguments("URLSearchParams.append", arguments.length, 2);
    this.#params.push([String(name), String(value)]);
    this.#updateSteps();
  }

  delete(name: string): void {
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
    this.#updateSteps();
  }

  getAll(name: string): string[] {
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

  get(name: string): string | null {
    requiredArguments("URLSearchParams.get", arguments.length, 1);
    name = String(name);
    for (const entry of this.#params) {
      if (entry[0] === name) {
        return entry[1];
      }
    }

    return null;
  }

  has(name: string): boolean {
    requiredArguments("URLSearchParams.has", arguments.length, 1);
    name = String(name);
    return this.#params.some((entry) => entry[0] === name);
  }

  set(name: string, value: string): void {
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
      this.append(name, value);
    }

    this.#updateSteps();
  }

  sort(): void {
    this.#params.sort((a, b) => (a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1));
    this.#updateSteps();
  }

  forEach(
    callbackfn: (value: string, key: string, parent: this) => void,
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    thisArg?: any
  ): void {
    requiredArguments("URLSearchParams.forEach", arguments.length, 1);

    if (typeof thisArg !== "undefined") {
      callbackfn = callbackfn.bind(thisArg);
    }

    for (const [key, value] of this.entries()) {
      callbackfn(value, key, this);
    }
  }

  *keys(): IterableIterator<string> {
    for (const [key] of this.#params) {
      yield key;
    }
  }

  *values(): IterableIterator<string> {
    for (const [, value] of this.#params) {
      yield value;
    }
  }

  *entries(): IterableIterator<[string, string]> {
    yield* this.#params;
  }

  *[Symbol.iterator](): IterableIterator<[string, string]> {
    yield* this.#params;
  }

  toString(): string {
    return this.#params
      .map(
        (tuple) =>
          `${encodeURIComponent(tuple[0])}=${encodeURIComponent(tuple[1])}`
      )
      .join("&");
  }
}
