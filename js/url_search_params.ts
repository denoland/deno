import { requiredArguments } from "./util";

// Copyright 2018 the Deno authors. All rights reserved. MIT license.
export class URLSearchParams {
  private params: Array<[string, string]> = [];

  constructor(init: string | string[][] | Record<string, string> = "") {
    if (typeof init === "string") {
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
        this.append(decodeURIComponent(name), decodeURIComponent(value));
      }
    } else if (Array.isArray(init)) {
      // Overload: sequence<sequence<USVString>>
      for (const tuple of init) {
        // If pair does not contain exactly two items, then throw a TypeError.
        if (tuple.length !== 2) {
          const errMsg =
            "Each query pair must be an iterable [name, value] tuple";
          throw new TypeError(errMsg);
        }
        this.append(tuple[0], tuple[1]);
      }
    } else if (Object(init) === init) {
      // Overload: record<USVString, USVString>
      for (const key of Object.keys(init)) {
        this.append(key, init[key]);
      }
    }
  }

  /** Appends a specified key/value pair as a new search parameter.
   *
   *       searchParams.append('name', 'first');
   *       searchParams.append('name', 'second');
   */
  append(name: string, value: string): void {
    requiredArguments("URLSearchParams.append", arguments.length, 2);
    this.params.push([String(name), value]);
  }

  /** Deletes the given search parameter and its associated value,
   * from the list of all search parameters.
   *
   *       searchParams.delete('name');
   */
  delete(name: string): void {
    requiredArguments("URLSearchParams.delete", arguments.length, 1);
    name = String(name);
    let i = 0;
    while (i < this.params.length) {
      if (this.params[i][0] === name) {
        this.params.splice(i, 1);
      } else {
        i++;
      }
    }
  }

  /** Returns all the values associated with a given search parameter
   * as an array.
   *
   *       searchParams.getAll('name');
   */
  getAll(name: string): string[] {
    requiredArguments("URLSearchParams.getAll", arguments.length, 1);
    name = String(name);
    const values = [];
    for (const entry of this.params) {
      if (entry[0] === name) {
        values.push(entry[1]);
      }
    }

    return values;
  }

  /** Returns the first value associated to the given search parameter.
   *
   *       searchParams.get('name');
   */
  get(name: string): string | null {
    requiredArguments("URLSearchParams.get", arguments.length, 1);
    name = String(name);
    for (const entry of this.params) {
      if (entry[0] === name) {
        return entry[1];
      }
    }

    return null;
  }

  /** Returns a Boolean that indicates whether a parameter with the
   * specified name exists.
   *
   *       searchParams.has('name');
   */
  has(name: string): boolean {
    requiredArguments("URLSearchParams.has", arguments.length, 1);
    name = String(name);
    return this.params.some(entry => entry[0] === name);
  }

  /** Sets the value associated with a given search parameter to the
   * given value. If there were several matching values, this method
   * deletes the others. If the search parameter doesn't exist, this
   * method creates it.
   *
   *       searchParams.set('name', 'value');
   */
  set(name: string, value: string): void {
    requiredArguments("URLSearchParams.set", arguments.length, 2);

    // If there are any name-value pairs whose name is name, in list,
    // set the value of the first such name-value pair to value
    // and remove the others.
    name = String(name);
    let found = false;
    let i = 0;
    while (i < this.params.length) {
      if (this.params[i][0] === name) {
        if (!found) {
          this.params[i][1] = value;
          found = true;
          i++;
        } else {
          this.params.splice(i, 1);
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
  }

  /** Sort all key/value pairs contained in this object in place and
   * return undefined. The sort order is according to Unicode code
   * points of the keys.
   *
   *       searchParams.sort();
   */
  sort(): void {
    this.params = this.params.sort((a, b) =>
      a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1
    );
  }

  /** Calls a function for each element contained in this object in
   * place and return undefined. Optionally accepts an object to use
   * as this when executing callback as second argument.
   *
   *       searchParams.forEach((value, key, parent) => {
   *         console.log(value, key, parent);
   *       });
   *
   */
  forEach(
    callbackfn: (value: string, key: string, parent: URLSearchParams) => void,
    // tslint:disable-next-line:no-any
    thisArg?: any
  ) {
    requiredArguments("URLSearchParams.forEach", arguments.length, 1);

    if (typeof thisArg !== "undefined") {
      callbackfn = callbackfn.bind(thisArg);
    }

    for (const [key, value] of this.entries()) {
      callbackfn(value, key, this);
    }
  }

  /** Returns an iterator allowing to go through all keys contained
   * in this object.
   *
   *       for (const key of searchParams.keys()) {
   *         console.log(key);
   *       }
   */
  *keys(): Iterable<string> {
    for (const entry of this.params) {
      yield entry[0];
    }
  }

  /** Returns an iterator allowing to go through all values contained
   * in this object.
   *
   *       for (const value of searchParams.values()) {
   *         console.log(value);
   *       }
   */
  *values(): Iterable<string> {
    for (const entry of this.params) {
      yield entry[1];
    }
  }

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   *       for (const [key, value] of searchParams.entries()) {
   *         console.log(key, value);
   *       }
   */
  *entries(): Iterable<[string, string]> {
    yield* this.params;
  }

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   *       for (const [key, value] of searchParams[Symbol.iterator]()) {
   *         console.log(key, value);
   *       }
   */
  *[Symbol.iterator](): Iterable<[string, string]> {
    yield* this.params;
  }

  /** Returns a query string suitable for use in a URL.
   *
   *        searchParams.toString();
   */
  toString(): string {
    return this.params
      .map(
        tuple =>
          `${encodeURIComponent(tuple[0])}=${encodeURIComponent(tuple[1])}`
      )
      .join("&");
  }
}
