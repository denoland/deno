// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { DomIterableMixin } from "./mixins/dom_iterable";

const params = Symbol("url_search_params array");

class URLSearchParamsBase {
  private [params]: Array<[string, string]> = [];

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
    this[params].push([name, value]);
  }

  /** Deletes the given search parameter and its associated value,
   * from the list of all search parameters.
   *
   *       searchParams.delete('name');
   */
  delete(name: string): void {
    let i = 0;
    while (i < this[params].length) {
      if (this[params][i][0] === name) {
        this[params].splice(i, 1);
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
    const values = [];
    for (const entry of this[params]) {
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
    for (const entry of this[params]) {
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
    return this[params].some(entry => entry[0] === name);
  }

  /** Sets the value associated with a given search parameter to the
   * given value. If there were several matching values, this method
   * deletes the others. If the search parameter doesn't exist, this
   * method creates it.
   *
   *       searchParams.set('name', 'value');
   */
  set(name: string, value: string): void {
    this.delete(name);
    this.append(name, value);
  }

  /** Sort all key/value pairs contained in this object in place and
   * return undefined. The sort order is according to Unicode code
   * points of the keys. The relative order between name-value pairs
   * with equal names must be preserved.
   *
   *       searchParams.sort();
   */
  // TODO(ztplz): The sort method of Array in v8 is not unstable. It may cause
  // the relative order of the array to be disordereduse, use mergesort
  // instead of it.
  sort(): void {
    this[params] = this[params].sort(
      (a, b) => (a[0] === b[0] ? 0 : a[0] > b[0] ? 1 : -1)
    );
  }

  /** Returns a query string suitable for use in a URL.
   *
   *        searchParams.toString();
   */
  toString(): string {
    return this[params]
      .map(
        tuple =>
          `${encodeURIComponent(tuple[0])}=${encodeURIComponent(tuple[1])}`
      )
      .join("&");
  }
}

// tslint:disable-next-line:variable-name
export const URLSearchParams = DomIterableMixin<
  string,
  string,
  typeof URLSearchParamsBase
>(URLSearchParamsBase, params);
